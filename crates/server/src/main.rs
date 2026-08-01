use std::{
    collections::HashMap,
    env,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context};
use axum::{
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::{Mutex, RwLock};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use waystation_shared::{
    candidates_for, fallback_version, fixture_reflection, fixture_response, passage,
    valid_selection, version_by_id, version_for_language, vignette, BibleVersion, HealthResponse,
    InterpretRequest, InterpretResponse, Passage, Provenance, ScriptureSource, DEFAULT_BIBLE_ID,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApiMode {
    Live,
    Fixture,
}

impl ApiMode {
    fn from_env() -> Self {
        match env::var("API_MODE").as_deref() {
            Ok("live") => Self::Live,
            _ => Self::Fixture,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Fixture => "fixture",
        }
    }
}

#[derive(Clone)]
struct Config {
    mode: ApiMode,
    gloo_client_id: Option<String>,
    gloo_client_secret: Option<String>,
    yvp_app_key: Option<String>,
    bible_id: u32,
    static_dir: PathBuf,
}

impl Config {
    fn from_env() -> Self {
        Self {
            mode: ApiMode::from_env(),
            gloo_client_id: env::var("GLOO_CLIENT_ID").ok(),
            gloo_client_secret: env::var("GLOO_CLIENT_SECRET").ok(),
            yvp_app_key: env::var("YVP_APP_KEY").ok(),
            bible_id: env::var("YVP_BIBLE_ID")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(DEFAULT_BIBLE_ID),
            static_dir: env::var("WAYSTATION_STATIC_DIR")
                .map_or_else(|_| PathBuf::from("dist"), PathBuf::from),
        }
    }

    /// Gloo alone is enough to go live. It is the half that decides anything —
    /// which need the traveler is voicing and which reviewed passage answers it.
    /// `YouVersion` only supplies the wording afterwards, and until a key exists
    /// the reviewed wording in `content/passages.ron` stands in for it.
    const fn live_ready(&self) -> bool {
        self.gloo_client_id.is_some() && self.gloo_client_secret.is_some()
    }
}

#[derive(Clone)]
struct AppState {
    config: Config,
    http: Client,
    token: Arc<Mutex<Option<CachedToken>>>,
    cache: Arc<RwLock<HashMap<String, InterpretResponse>>>,
}

#[derive(Clone)]
struct CachedToken {
    value: String,
    refresh_at: Instant,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default = "default_token_lifetime")]
    expires_in: u64,
}

const fn default_token_lifetime() -> u64 {
    3_600
}

#[derive(Debug, Deserialize)]
struct ToolSelection {
    need_id: String,
    passage_id: String,
    reflection: String,
}

#[derive(Debug)]
struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        tracing::error!(error = %self.0, "request failed");
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": "The old wires are quiet. Please try again."})),
        )
            .into_response()
    }
}

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>,
{
    fn from(value: E) -> Self {
        Self(value.into())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "waystation_server=info,tower_http=info".into()),
        )
        .init();

    let config = Config::from_env();
    if config.mode == ApiMode::Live && !config.live_ready() {
        return Err(anyhow!(
            "API_MODE=live requires GLOO_CLIENT_ID and GLOO_CLIENT_SECRET"
        ));
    }

    let address: SocketAddr = env::var("WAYSTATION_BIND")
        .unwrap_or_else(|_| "0.0.0.0:7777".to_owned())
        .parse()
        .context("invalid WAYSTATION_BIND")?;
    let state = AppState {
        config: config.clone(),
        http: Client::builder()
            .timeout(Duration::from_secs(12))
            .user_agent("Waystation/0.1 scripture-in-new-frontiers")
            .build()?,
        token: Arc::new(Mutex::new(None)),
        cache: Arc::new(RwLock::new(HashMap::new())),
    };
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(
        %address,
        mode = config.mode.label(),
        scripture = if config.yvp_app_key.is_some() { "youversion" } else { "reviewed-local" },
        "waystation server listening"
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn build_router(state: AppState) -> Router {
    let index = state.config.static_dir.join("index.html");
    // Missing Bevy `.meta` sidecars must remain 404s. Falling every unknown asset
    // back to index.html makes Bevy try to parse HTML as asset metadata and then
    // reject the corresponding texture.
    let static_files = ServeDir::new(state.config.static_dir.clone());
    Router::new()
        .route("/api/health", get(health))
        .route("/api/interpret", post(interpret))
        .route_service("/", ServeFile::new(index))
        .fallback_service(static_files)
        .layer(DefaultBodyLimit::max(4 * 1_024))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_owned(),
        api_mode: state.config.mode.label().to_owned(),
        gloo_configured: state.config.gloo_client_id.is_some(),
        youversion_configured: state.config.yvp_app_key.is_some(),
    })
}

async fn interpret(
    State(state): State<AppState>,
    Json(request): Json<InterpretRequest>,
) -> Result<Json<InterpretResponse>, ApiError> {
    let vignette = vignette(&request.vignette_id)
        .ok_or_else(|| anyhow!("unknown vignette id: {}", request.vignette_id))?;

    if state.config.mode == ApiMode::Fixture {
        return fixture_response(&vignette.id)
            .map(Json)
            .ok_or_else(|| anyhow!("fixture missing").into());
    }

    // The cache is keyed by translation as well as vignette. Keyed by vignette
    // alone, the first player through would decide what language everyone after
    // them heard.
    let version = requested_version(&state, request.language.as_deref());
    let cache_key = format!("{}@{}", vignette.id, version.id);

    match interpret_live(&state, vignette, version).await {
        Ok(response) => {
            state
                .cache
                .write()
                .await
                .insert(cache_key, response.clone());
            Ok(Json(response))
        }
        Err(error) => {
            tracing::warn!(%error, vignette_id = vignette.id, "live APIs failed; using reviewed fallback");
            let cached_response = state.cache.read().await.get(&cache_key).cloned();
            if let Some(mut response) = cached_response {
                response.provenance.scripture_source = ScriptureSource::Cache;
                return Ok(Json(response));
            }
            let mut response = fixture_response(&vignette.id)
                .ok_or_else(|| anyhow!("fallback fixture missing for {}", vignette.id))?;
            "reviewed-fallback-after-live-error".clone_into(&mut response.provenance.routing);
            Ok(Json(response))
        }
    }
}

/// The translation a request asks for, or the reviewed English one.
///
/// `YVP_BIBLE_ID` still names the version to use when a player's language has no
/// entry, so a deployment can pick a different English edition without a code
/// change; it is looked up in the catalog so the abbreviation reported to the
/// player is the version's own and not an assumption.
fn requested_version(state: &AppState, language: Option<&str>) -> &'static BibleVersion {
    language
        .and_then(version_for_language)
        .or_else(|| version_by_id(state.config.bible_id))
        .unwrap_or_else(fallback_version)
}

async fn interpret_live(
    state: &AppState,
    vignette: &'static waystation_shared::Vignette,
    version: &'static BibleVersion,
) -> anyhow::Result<InterpretResponse> {
    let token = access_token(state).await?;
    let candidates = candidates_for(vignette);
    let candidate_ids: Vec<&str> = candidates.iter().map(|item| item.id.as_str()).collect();
    let need_ids: Vec<&str> = vignette.needs.iter().map(String::as_str).collect();
    let transcript = vignette.lines.join("\n");
    let prompt = format!(
        "You are assisting a compassionate, ecumenical fictional scribe. Interpret the human need in this authored traveler vignette. Select only an allowed need and a passage paired with that need. Write one gentle reflection under 140 characters. Do not quote, paraphrase, preach, promise an outcome, or attempt conversion.\n\nTraveler:\n{transcript}"
    );
    let body = json!({
        "auto_routing": true,
        "stream": false,
        "temperature": 0.2,
        "max_tokens": 220,
        "messages": [
            {"role": "system", "content": "Return the selection by calling select_remembrance exactly once."},
            {"role": "user", "content": prompt}
        ],
        "tools": [{
            "type": "function",
            "function": {
                "name": "select_remembrance",
                "description": "Select a reviewed need and Scripture reference for an authored vignette.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["need_id", "passage_id", "reflection"],
                    "properties": {
                        "need_id": {"type": "string", "enum": need_ids},
                        "passage_id": {"type": "string", "enum": candidate_ids},
                        "reflection": {"type": "string", "maxLength": 140}
                    }
                }
            }
        }],
        "tool_choice": "required"
    });
    let completion: Value = state
        .http
        .post("https://platform.ai.gloo.com/ai/v2/chat/completions")
        .bearer_auth(token)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let arguments = tool_arguments(&completion)?;
    let selection: ToolSelection = serde_json::from_str(arguments)?;
    if !valid_selection(vignette, &selection.need_id, &selection.passage_id) {
        return Err(anyhow!("Gloo selected a disallowed need/passage pair"));
    }
    if !valid_reflection(&selection.reflection) {
        return Err(anyhow!("Gloo reflection failed length validation"));
    }
    let candidate = passage(&selection.passage_id).context("validated passage disappeared")?;
    let (scripture, scripture_source) = resolve_passage(state, candidate, version).await?;
    let model = completion
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("gloo-auto-routing")
        .to_owned();
    let routing = completion
        .get("routing_mechanism")
        .and_then(Value::as_str)
        .unwrap_or("auto_routing")
        .to_owned();

    Ok(InterpretResponse {
        vignette_id: vignette.id.clone(),
        need_id: candidate.need_id.clone(),
        need_label: candidate.need_label.clone(),
        reflection: if selection.reflection.trim().is_empty() {
            fixture_reflection(&candidate.need_id).to_owned()
        } else {
            selection.reflection.trim().to_owned()
        },
        passage: scripture,
        provenance: Provenance {
            gloo_model: model,
            routing,
            scripture_source,
        },
    })
}

/// Where the words themselves come from. A missing `YouVersion` key is not a
/// failure — Gloo has already done the choosing, and the reviewed wording is
/// what the fixture would have served anyway. What must never happen is
/// reporting `YouVersion` as the source of text `YouVersion` did not send.
async fn resolve_passage(
    state: &AppState,
    candidate: &'static waystation_shared::PassageCandidate,
    version: &'static BibleVersion,
) -> anyhow::Result<(Passage, ScriptureSource)> {
    if state.config.yvp_app_key.is_none() {
        return Ok((reviewed_passage(candidate), ScriptureSource::ReviewedLocal));
    }
    let scripture = fetch_passage(state, &candidate.id, version).await?;
    Ok((scripture, ScriptureSource::YouVersionLive))
}

/// The offline stand-in is the one English wording a human reviewed, whatever
/// language was asked for. It is labelled English for that reason: a Spanish
/// player is better served by visible English than by English under a Spanish
/// name.
fn reviewed_passage(candidate: &'static waystation_shared::PassageCandidate) -> Passage {
    let version = fallback_version();
    Passage {
        id: candidate.id.clone(),
        reference: candidate.reference.clone(),
        content: candidate.fixture_content.clone(),
        version: version.abbreviation.clone(),
        youversion_deep_link: format!(
            "https://www.bible.com/bible/{}/{}",
            version.id, candidate.id
        ),
    }
}

/// Gloo's content controls answer with a plain `200` and prose where the tool
/// call should be, so a refused vignette is indistinguishable from a network
/// fault unless the refusal is read back. Both end at the reviewed fallback;
/// only one of them is worth editing a vignette over.
fn tool_arguments(completion: &Value) -> anyhow::Result<&str> {
    if let Some(arguments) = completion
        .pointer("/choices/0/message/tool_calls/0/function/arguments")
        .and_then(Value::as_str)
    {
        return Ok(arguments);
    }
    let refusal = completion
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty());
    refusal.map_or_else(
        || {
            Err(anyhow!(
                "Gloo response did not contain the required tool call"
            ))
        },
        |text| {
            Err(anyhow!(
                "Gloo content controls declined the vignette: {text}"
            ))
        },
    )
}

fn valid_reflection(reflection: &str) -> bool {
    let trimmed = reflection.trim();
    !trimmed.is_empty()
        && trimmed.chars().count() <= 140
        && !trimmed
            .chars()
            .any(|character| matches!(character, '"' | '“' | '”'))
}

async fn access_token(state: &AppState) -> anyhow::Result<String> {
    {
        let cached = state.token.lock().await;
        if let Some(token) = cached
            .as_ref()
            .filter(|token| token.refresh_at > Instant::now())
        {
            return Ok(token.value.clone());
        }
    }
    let id = state
        .config
        .gloo_client_id
        .as_deref()
        .context("missing Gloo client id")?;
    let secret = state
        .config
        .gloo_client_secret
        .as_deref()
        .context("missing Gloo client secret")?;
    let response: TokenResponse = state
        .http
        .post("https://platform.ai.gloo.com/oauth2/token")
        .basic_auth(id, Some(secret))
        .form(&[("grant_type", "client_credentials")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let refresh_seconds = response.expires_in.saturating_sub(120).max(30);
    {
        let mut cached = state.token.lock().await;
        *cached = Some(CachedToken {
            value: response.access_token.clone(),
            refresh_at: Instant::now() + Duration::from_secs(refresh_seconds),
        });
    }
    Ok(response.access_token)
}

async fn fetch_passage(
    state: &AppState,
    passage_id: &str,
    version: &'static BibleVersion,
) -> anyhow::Result<Passage> {
    let key = state
        .config
        .yvp_app_key
        .as_deref()
        .context("missing YouVersion app key")?;
    let url = format!(
        "https://api.youversion.com/v1/bibles/{}/passages/{passage_id}",
        version.id
    );
    let payload: Value = state
        .http
        .get(&url)
        .header("X-YVP-App-Key", key)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let data = payload.get("data").unwrap_or(&payload);
    let content = data
        .get("content")
        .and_then(Value::as_str)
        .context("YouVersion response missing passage content")?;
    let reference = data
        .get("reference")
        .and_then(Value::as_str)
        .unwrap_or(passage_id);
    Ok(Passage {
        id: passage_id.to_owned(),
        reference: reference.to_owned(),
        content: strip_simple_html(content),
        version: version.abbreviation.clone(),
        youversion_deep_link: format!("https://www.bible.com/bible/{}/{passage_id}", version.id),
    })
}

fn strip_simple_html(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;
    for character in input.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }
    output
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use waystation_shared::bible_versions;

    #[test]
    fn strips_basic_html_from_passages() {
        assert_eq!(
            strip_simple_html("<p>The <span class=\"wj\">old words</span> remain.</p>"),
            "The old words remain."
        );
    }

    #[test]
    fn rejects_quoted_or_oversized_reflections() {
        assert!(valid_reflection("A short compassionate reflection."));
        assert!(!valid_reflection("“An invented quotation.”"));
        assert!(!valid_reflection(&"x".repeat(141)));
    }

    fn state_with(mode: ApiMode, yvp_app_key: Option<&str>) -> AppState {
        AppState {
            config: Config {
                mode,
                gloo_client_id: None,
                gloo_client_secret: None,
                yvp_app_key: yvp_app_key.map(str::to_owned),
                bible_id: DEFAULT_BIBLE_ID,
                static_dir: PathBuf::from("dist"),
            },
            http: Client::new(),
            token: Arc::new(Mutex::new(None)),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Gloo is the half that decides; `YouVersion` only supplies wording. Coupling
    /// them meant one missing key kept the whole live path switched off.
    #[test]
    fn live_mode_needs_gloo_but_not_youversion() {
        let mut config = state_with(ApiMode::Live, None).config;
        assert!(!config.live_ready());

        config.gloo_client_id = Some("id".to_owned());
        config.gloo_client_secret = Some("secret".to_owned());
        assert!(config.live_ready());
    }

    /// Without a `YouVersion` key the passage still has to arrive, and it has to
    /// arrive labelled as what it is. Reporting ``YouVersion`Live` over reviewed
    /// local wording would put a false credit on the traveler's card.
    #[tokio::test]
    async fn a_missing_youversion_key_serves_reviewed_wording_under_its_own_name() {
        let state = state_with(ApiMode::Live, None);
        let candidate = passage("PSA.34.18").unwrap();

        let (scripture, source) = resolve_passage(&state, candidate, fallback_version())
            .await
            .unwrap();

        assert_eq!(source, ScriptureSource::ReviewedLocal);
        assert_eq!(scripture.content, candidate.fixture_content);
        assert_eq!(scripture.reference, "Psalm 34:18");
        assert!(scripture.youversion_deep_link.ends_with("/PSA.34.18"));
    }

    /// Gloo's content controls answer `200 OK` with prose instead of the tool
    /// call, so the refusal has to be read back out of the body or an edited
    /// vignette looks exactly like an unplugged network.
    #[test]
    fn a_content_control_refusal_is_reported_as_itself() {
        let refused = json!({"choices": [{"message": {
            "content": "I'm sorry, but I cannot assist with that request.",
            "tool_calls": null
        }}]});
        let error = tool_arguments(&refused).unwrap_err().to_string();
        assert!(error.contains("content controls declined"), "{error}");
        assert!(error.contains("I cannot assist"), "{error}");

        let empty = json!({"choices": [{"message": {"content": "", "tool_calls": null}}]});
        assert!(tool_arguments(&empty)
            .unwrap_err()
            .to_string()
            .contains("did not contain the required tool call"));

        let selected = json!({"choices": [{"message": {"tool_calls": [
            {"function": {"name": "select_remembrance", "arguments": "{}"}}
        ]}}]});
        assert_eq!(tool_arguments(&selected).unwrap(), "{}");
    }

    /// A player's language picks the translation; anything unmapped lands on
    /// English rather than on nothing.
    #[test]
    fn a_players_language_chooses_their_translation() {
        let state = state_with(ApiMode::Live, Some("key"));

        let spanish = requested_version(&state, Some("es-MX"));
        assert_eq!(spanish.language, "es");
        assert_ne!(spanish.id, fallback_version().id);

        for absent in [None, Some(""), Some("zzz")] {
            assert_eq!(
                requested_version(&state, absent).id,
                fallback_version().id,
                "{absent:?}"
            );
        }
    }

    /// `YVP_BIBLE_ID` still names the edition for players whose language has no
    /// entry, and the abbreviation reported comes from the catalog rather than
    /// from an assumption about which version that id is.
    #[tokio::test]
    async fn the_configured_edition_answers_for_unmapped_languages() {
        let mut state = state_with(ApiMode::Live, None);
        let asv = bible_versions()
            .iter()
            .find(|version| version.abbreviation == "ASV")
            .expect("the catalog offers ASV among the English alternatives");
        state.config.bible_id = asv.id;

        assert_eq!(requested_version(&state, Some("zzz")).abbreviation, "ASV");
        assert_eq!(requested_version(&state, Some("es")).language, "es");
    }

    /// The offline stand-in is English whatever was asked for, and has to say
    /// so. Labelling reviewed English as a Spanish edition would put a false
    /// translation credit on the card.
    #[tokio::test]
    async fn reviewed_wording_is_labelled_english_even_when_spanish_was_asked_for() {
        let state = state_with(ApiMode::Live, None);
        let candidate = passage("PSA.34.18").unwrap();
        let spanish = version_for_language("es").expect("Spanish is mapped");

        let (scripture, source) = resolve_passage(&state, candidate, spanish).await.unwrap();

        assert_eq!(source, ScriptureSource::ReviewedLocal);
        assert_eq!(scripture.version, fallback_version().abbreviation);
        assert_eq!(scripture.content, candidate.fixture_content);
    }

    #[tokio::test]
    async fn fixture_endpoint_returns_traceable_scripture() {
        let state = state_with(ApiMode::Fixture, None);
        let response = build_router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/interpret")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"vignette_id":"mara_grief"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let payload: InterpretResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(payload.passage.id, "PSA.34.18");
        assert_eq!(
            payload.provenance.scripture_source,
            ScriptureSource::Fixture
        );
    }
}
