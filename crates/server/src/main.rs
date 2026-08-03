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
    body::Body,
    extract::{DefaultBodyLimit, State},
    http::{header, HeaderValue, Method, Request, Response, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::{Mutex, RwLock};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
    validate_request::{ValidateRequest, ValidateRequestHeaderLayer},
};
use waystation_shared::{
    candidates_for, card_catalogue, card_print, fallback_version, fixture_reflection,
    fixture_response, passage, valid_selection, version_by_id, version_for_language, vignette,
    BibleVersion, CardPrint, CardRequest, CardResponse, HealthResponse, InterpretRequest,
    InterpretResponse, Passage, Provenance, ScriptureSource, DEFAULT_BIBLE_ID,
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
    gate: Option<Credential>,
    /// Origins other than our own that may reach the API, from
    /// `WAYSTATION_ALLOWED_ORIGINS` as a comma-separated list.
    allowed_origins: Vec<String>,
}

/// One shared username and password for the hosted demo.
///
/// The browser build serves purchased art that may not be redistributed, and a
/// public URL would be publishing it. This is a door, not a security boundary:
/// anyone let through can save the files, as they can from any game that runs in
/// a browser. What it buys is the difference between art that is served and art
/// that is published, which is the difference the licences care about.
#[derive(Clone)]
struct Credential {
    /// The complete `Authorization` header the door will accept, folded once at
    /// startup so that answering the knock is a string comparison.
    expected: String,
}

impl Credential {
    /// Reads `WAYSTATION_GATE`, as `user:password`. Unset — which is how local
    /// development and the container's own defaults leave it — the road is open.
    fn from_env() -> Option<Self> {
        Self::parse(&env::var("WAYSTATION_GATE").ok()?)
    }

    /// A half-written credential is refused rather than half-applied. `user:` is
    /// the shape a shell leaves behind when the secret it should have expanded to
    /// was never set, and honouring it would open the door while logging it shut.
    ///
    /// Only the username is taken to end at the first colon; a password may
    /// contain them, and basic authentication encodes the pair whole.
    fn parse(value: &str) -> Option<Self> {
        let (username, password) = value.split_once(':')?;
        (!username.is_empty() && !password.is_empty()).then(|| Self {
            expected: format!("Basic {}", BASE64.encode(value)),
        })
    }
}

/// Compares in time that does not depend on how much of the credential was
/// right, so that repeated knocking cannot be used to read the password back one
/// character at a time.
fn matches_in_constant_time(offered: &[u8], expected: &[u8]) -> bool {
    if offered.len() != expected.len() {
        return false;
    }
    offered
        .iter()
        .zip(expected)
        .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

impl<B> ValidateRequest<B> for Credential {
    type ResponseBody = Body;

    fn validate(&mut self, request: &mut Request<B>) -> Result<(), Response<Body>> {
        let offered = request
            .headers()
            .get(header::AUTHORIZATION)
            .map(HeaderValue::as_bytes)
            .unwrap_or_default();
        if matches_in_constant_time(offered, self.expected.as_bytes()) {
            return Ok(());
        }
        Err(Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(header::WWW_AUTHENTICATE, r#"Basic realm="Waystation""#)
            .body(Body::from("The waystation is not open to the road yet.\n"))
            .expect("the refusal is built from constants"))
    }
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
            gate: Credential::from_env(),
            allowed_origins: env::var("WAYSTATION_ALLOWED_ORIGINS")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(str::to_owned)
                .collect(),
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
    /// Unlike a vignette, a card has exactly one right answer per translation:
    /// the same phrase of the same verse, every time. So this one is read
    /// through rather than kept for a rainy day — a second traveler reading
    /// Spanish costs nothing.
    cards: Arc<RwLock<HashMap<String, CardResponse>>>,
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
struct SpanSelection {
    span: String,
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
        cards: Arc::new(RwLock::new(HashMap::new())),
    };
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(
        %address,
        mode = config.mode.label(),
        scripture = if config.yvp_app_key.is_some() { "youversion" } else { "reviewed-local" },
        door = if config.gate.is_some() { "credentialed" } else { "open" },
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
    let waystation = Router::new()
        .route("/api/interpret", post(interpret))
        .route("/api/card", post(card))
        .route_service("/", ServeFile::new(index))
        .fallback_service(static_files);
    // Health is deliberately outside the gate. The platform's own checks carry no
    // credentials, and a 401 there reads to them as a machine that has died.
    let waystation = match state.config.gate.clone() {
        Some(gate) => waystation.layer(ValidateRequestHeaderLayer::custom(gate)),
        None => waystation,
    };
    let router = Router::new()
        .route("/api/health", get(health))
        .merge(waystation)
        .layer(DefaultBodyLimit::max(4 * 1_024))
        .layer(TraceLayer::new_for_http());
    // Outermost on purpose. A preflight carries no credentials, so a gate placed
    // above this one would refuse the question the browser has to ask before it
    // will carry the real request.
    let router = match cross_origin(&state.config.allowed_origins) {
        Some(cors) => router.layer(cors),
        None => router,
    };
    router.with_state(state)
}

/// Permission for the submitted demo link to reach this API.
///
/// The link in the submission is a static host and cannot be moved. It has no
/// `/api/interpret` of its own, so the game asks this server for one, and the
/// browser will not carry a request across origins until the far side says it
/// may. Named origins only: a wildcard here would let any page on the internet
/// spend our Gloo quota.
///
/// Unset, no header is sent at all and the server answers only the page it
/// serves itself, which is what a local `make server-live` wants.
fn cross_origin(origins: &[String]) -> Option<CorsLayer> {
    let allowed: Vec<HeaderValue> = origins
        .iter()
        .filter_map(|origin| HeaderValue::from_str(origin).ok())
        .collect();
    if allowed.is_empty() {
        return None;
    }
    Some(
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(allowed))
            .allow_methods([Method::GET, Method::POST])
            .allow_headers([header::ACCEPT, header::CONTENT_TYPE]),
    )
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

/// The most a block can hold, counted in characters rather than words because
/// not every script has words in the sense a space would settle. A span longer
/// than this would draw over the illustration, so it is refused and the card is
/// handed over in the Scribe's own English instead — legible beats overflowing.
const CARD_EXCERPT_LIMIT: usize = 96;

/// The words for one card, in the language of whoever is being handed it.
///
/// The card is a physical thing with fixed words, so nothing here is composed:
/// the answer is always a literal span of a published verse, either the English
/// one already cut into the block or the matching span of the same verse in
/// another translation. What cannot be verified is not sent.
async fn card(
    State(state): State<AppState>,
    Json(request): Json<CardRequest>,
) -> Result<Json<CardResponse>, ApiError> {
    let print = card_print(&request.print_id)
        .ok_or_else(|| anyhow!("unknown print id: {}", request.print_id))?;
    let version = requested_version(&state, request.language.as_deref());
    // Nothing to do for a reader of the edition the blocks were cut from, and
    // nothing to do without the key that would fetch another one.
    if state.config.mode == ApiMode::Fixture
        || state.config.yvp_app_key.is_none()
        || version.id == card_catalogue().translation.youversion_id
    {
        return Ok(Json(card_as_cut(print)));
    }

    let cache_key = format!("{}@{}", print.id, version.id);
    // Bound before the branch: a read guard held across the branch body would
    // still be alive when the miss path takes the write lock.
    let known = state.cards.read().await.get(&cache_key).cloned();
    if let Some(known) = known {
        return Ok(Json(known));
    }
    match translated_card(&state, print, version).await {
        Ok(response) => {
            state
                .cards
                .write()
                .await
                .insert(cache_key, response.clone());
            Ok(Json(response))
        }
        Err(error) => {
            tracing::warn!(
                %error,
                print_id = print.id,
                version = version.abbreviation,
                "no verifiable span; handing the card over as it was cut"
            );
            Ok(Json(card_as_cut(print)))
        }
    }
}

/// The card exactly as it is printed. Labelled with the edition it was cut from,
/// for the same reason `reviewed_passage` is: a Spanish reader is better served
/// by visible English than by English under a Spanish name.
fn card_as_cut(print: &'static CardPrint) -> CardResponse {
    CardResponse {
        print_id: print.id.clone(),
        reference: print.reference.clone(),
        excerpt: print.excerpt.clone(),
        version: card_catalogue().translation.name.clone(),
    }
}

async fn translated_card(
    state: &AppState,
    print: &'static CardPrint,
    version: &'static BibleVersion,
) -> anyhow::Result<CardResponse> {
    let whole = fetch_passage(state, &print.passage_id, version).await?;
    let excerpt = matching_span(state, print, &whole, version).await?;
    Ok(CardResponse {
        print_id: print.id.clone(),
        reference: whole.reference,
        excerpt,
        version: version.abbreviation.clone(),
    })
}

/// Which run of the published verse says what the block says.
///
/// A model is asked to point, never to write. Whatever comes back is held to
/// being a literal contiguous span of the text that was sent, so the words on
/// the card are the translators' own — a quotation, not a machine's rendering of
/// Scripture. A model that paraphrases, corrects, or invents fails the check and
/// the card falls back to English, which is a card nobody has to take on trust.
async fn matching_span(
    state: &AppState,
    print: &'static CardPrint,
    whole: &Passage,
    version: &'static BibleVersion,
) -> anyhow::Result<String> {
    let token = access_token(state).await?;
    let prompt = format!(
        "Here is {reference} as published in {title}:\n\n{content}\n\nAn English \
         edition of the same verse carries this phrase: \u{201c}{excerpt}\u{201d}\n\n\
         Point at the shortest unbroken run of the text above that says what that \
         phrase says. Copy it out exactly as it appears there, character for \
         character, starting and ending at a word boundary. Do not translate, \
         correct, reorder, shorten a word, or add punctuation that is not there. \
         If no unbroken run of it carries that meaning, return an empty string.",
        reference = whole.reference,
        title = version.title,
        content = whole.content,
        excerpt = print.excerpt,
    );
    let body = json!({
        "auto_routing": true,
        "stream": false,
        "temperature": 0.0,
        "max_tokens": 220,
        "messages": [
            {"role": "system", "content": "Return the span by calling select_span exactly once. The span must be copied verbatim from the verse in the user message."},
            {"role": "user", "content": prompt}
        ],
        "tools": [{
            "type": "function",
            "function": {
                "name": "select_span",
                "description": "Quote one unbroken run of the supplied verse.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["span"],
                    "properties": {"span": {"type": "string"}}
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
    let offered: SpanSelection = serde_json::from_str(arguments)?;
    verified_span(&whole.content, &offered.span).ok_or_else(|| {
        anyhow!(
            "{}: offered span is not a run of {} in {}",
            print.id,
            whole.reference,
            version.abbreviation
        )
    })
}

/// The whole of the trust in this path, and deliberately small enough to read.
///
/// Quote marks a model wrapped around its answer are its own and come off; after
/// that the text either occurs in the verse or it does not, and nothing that
/// does not occur in the verse goes on a card.
fn verified_span(whole: &str, offered: &str) -> Option<String> {
    let span = offered
        .trim()
        .trim_matches(|mark| matches!(mark, '"' | '\u{201c}' | '\u{201d}'))
        .trim();
    if span.is_empty() || span.chars().count() > CARD_EXCERPT_LIMIT {
        return None;
    }
    whole.contains(span).then(|| span.to_owned())
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
        content: clean_passage_text(content),
        version: version.abbreviation.clone(),
        youversion_deep_link: format!("https://www.bible.com/bible/{}/{passage_id}", version.id),
    })
}

/// The wording as it should read on a traveler's card.
///
/// The same tidying `scripts/fetch-verses.py` does for the printed cards, so a
/// passage that arrives at runtime reads exactly as one baked in at build time.
/// Doing less here would mean the live path is the one that shows its seams.
fn clean_passage_text(input: &str) -> String {
    let without_markup = drop_editorial_marks(&strip_simple_html(input));
    let collapsed = without_markup
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    close_the_quotation(&collapsed).to_owned()
}

/// Drops a quotation mark that has nothing to pair with.
///
/// `MAT.11.28-30` ends `…and My burden is light.”` — the passage is a cut out of
/// a longer speech and the mark that opened it lies in a verse nobody asked for.
/// The card puts its own quotation marks around whatever it is handed, so the
/// stray one arrives as `light.””`. A mark at the very edge with no partner
/// anywhere in the text belongs to the cut rather than to the words, and only
/// that one is taken; a passage whose quotation opens and closes within itself
/// is punctuation somebody wrote and is left alone.
fn close_the_quotation(text: &str) -> &str {
    let opened = text.matches('\u{201c}').count();
    let closed = text.matches('\u{201d}').count();
    if closed > opened {
        if let Some(trimmed) = text.strip_suffix('\u{201d}') {
            return trimmed;
        }
    }
    if opened > closed {
        if let Some(trimmed) = text.strip_prefix('\u{201c}') {
            return trimmed;
        }
    }
    text
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
}

/// Drops the editor's marks and keeps the editor's words.
///
/// `Matthew 6:34` comes back ending `own.[’’]` — a bracketed note about where a
/// quotation mark belongs, not Scripture. A bracketed group holding any letter
/// or digit is a textual-doubt marker around real words, and cutting it would
/// change what the verse says, so only the wordless groups go.
fn drop_editorial_marks(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(open) = rest.find('[') {
        let (before, from_bracket) = rest.split_at(open);
        output.push_str(before);
        let Some(close) = from_bracket.find(']') else {
            // An unclosed bracket is not a mark, it is part of the text.
            rest = from_bracket;
            break;
        };
        let inside = &from_bracket[1..close];
        if inside
            .chars()
            .any(|character| character.is_alphanumeric() || character == '_')
        {
            output.push_str(&from_bracket[..=close]);
        }
        rest = &from_bracket[close + 1..];
    }
    output.push_str(rest);
    output
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{header, Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use waystation_shared::bible_versions;

    #[test]
    fn strips_basic_html_from_passages() {
        assert_eq!(
            clean_passage_text("<p>The <span class=\"wj\">old words</span> remain.</p>"),
            "The old words remain."
        );
    }

    /// `scripts/fetch-verses.py` learned this from the API for the printed
    /// cards; the runtime path serves the same verses and has to read the same.
    #[test]
    fn editorial_marks_go_and_bracketed_words_stay() {
        assert_eq!(
            clean_passage_text("<p>trouble of its own.[’’]</p>"),
            "trouble of its own."
        );
        assert_eq!(clean_passage_text("a mark [*] here"), "a mark here");
        assert_eq!(
            clean_passage_text("<p>the Lord [of hosts] spoke</p>"),
            "the Lord [of hosts] spoke"
        );
        // An unclosed bracket is text, not a mark, and survives whole.
        assert_eq!(clean_passage_text("half [a bracket"), "half [a bracket");
    }

    /// One of the six passages a traveler can be handed is a cut out of a longer
    /// speech, and arrives carrying the mark that closed it. The card supplies
    /// its own quotation marks, so left alone that reads `light.””`.
    #[test]
    fn a_quotation_mark_with_nothing_to_pair_with_is_not_punctuation() {
        assert_eq!(
            clean_passage_text("For My yoke is easy and My burden is light.\u{201d}"),
            "For My yoke is easy and My burden is light."
        );
        assert_eq!(
            clean_passage_text("\u{201c}Come to Me, all you who are weary"),
            "Come to Me, all you who are weary"
        );
        // A quotation that opens and closes inside the passage is somebody's
        // punctuation and stays exactly as it was written.
        let whole = "He said, \u{201c}Peace be with you.\u{201d}";
        assert_eq!(clean_passage_text(whole), whole);
        // Only the mark at the edge goes; one buried mid-sentence is speech.
        let inner = "then he said \u{201d} and stopped";
        assert_eq!(clean_passage_text(inner), inner);
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
                gate: None,
                allowed_origins: Vec::new(),
            },
            http: Client::new(),
            token: Arc::new(Mutex::new(None)),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cards: Arc::new(RwLock::new(HashMap::new())),
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

    /// The whole safety of a translated card. A model is asked to point at a
    /// run of published text; anything that is not literally there is refused,
    /// so no card can ever carry a machine's own wording of Scripture.
    #[test]
    fn only_words_that_are_literally_in_the_verse_can_go_on_a_card() {
        const VERSE: &str = "Vengan a mí todos ustedes que están cansados y \
                             agobiados, y yo les daré descanso.";

        assert_eq!(
            verified_span(VERSE, "yo les daré descanso"),
            Some("yo les daré descanso".to_owned())
        );
        // A model that wrapped its answer in quotation marks still pointed at
        // the right words.
        assert_eq!(
            verified_span(VERSE, " \u{201c}yo les daré descanso\u{201d} "),
            Some("yo les daré descanso".to_owned())
        );
        // A paraphrase, a correction and a different translation are all the
        // same failure: text nobody published.
        for invented in [
            "yo os daré descanso",
            "Yo Les Daré Descanso",
            "I will give you rest",
            "",
            "   ",
        ] {
            assert_eq!(verified_span(VERSE, invented), None, "{invented:?}");
        }
    }

    /// A span long enough to draw over the illustration is no use on a card, and
    /// a block cut by hand would not carry it either.
    #[test]
    fn a_span_too_long_to_cut_is_refused_like_any_other() {
        let sprawling = "a ".repeat(CARD_EXCERPT_LIMIT);
        let whole = format!("Before {sprawling} after");

        assert!(verified_span(&whole, &sprawling).is_none());
        assert!(verified_span(&whole, "Before").is_some());
    }

    async fn card_for(state: AppState, body: &str) -> CardResponse {
        let response = build_router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/card")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_owned()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    /// Without a key to fetch another edition there is nothing to choose from,
    /// so the card goes over as it was cut — under the name of the edition it
    /// was cut from, never under a Spanish one.
    #[tokio::test]
    async fn a_card_nobody_can_translate_is_handed_over_as_it_was_cut() {
        let payload = card_for(
            state_with(ApiMode::Fixture, None),
            r#"{"print_id":"early-hospitality","language":"es"}"#,
        )
        .await;

        let cut = card_print("early-hospitality").expect("a catalogued card");
        assert_eq!(payload.excerpt, cut.excerpt);
        assert_eq!(payload.reference, cut.reference);
        assert_eq!(payload.version, card_catalogue().translation.name);
    }

    /// An English reader is asking for the words already on the block, so
    /// nothing is fetched and nothing is chosen.
    #[tokio::test]
    async fn a_card_in_its_own_edition_costs_nothing_to_answer() {
        let payload = card_for(
            state_with(ApiMode::Live, Some("test-key")),
            r#"{"print_id":"early-rest","language":"en"}"#,
        )
        .await;

        assert_eq!(payload.excerpt, "I will give you rest");
        assert_eq!(payload.version, card_catalogue().translation.name);
    }

    /// The catalogue is the whole of what can be asked for. A server that
    /// fetched any reference it was handed would be a public proxy to somebody
    /// else's licensed text, on our key.
    #[tokio::test]
    async fn a_card_that_was_never_cut_is_not_a_way_to_ask_for_a_verse() {
        let response = build_router(state_with(ApiMode::Live, Some("test-key")))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/card")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"print_id":"JHN.3.16","language":"es"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    fn gated_state() -> AppState {
        let mut state = state_with(ApiMode::Fixture, None);
        state.config.gate = Credential::parse("judge:let-me-in");
        state
    }

    async fn status_of(state: AppState, uri: &str, credential: Option<&str>) -> StatusCode {
        let mut request = Request::builder().method("GET").uri(uri);
        if let Some(credential) = credential {
            let encoded = BASE64.encode(credential);
            request = request.header("authorization", format!("Basic {encoded}"));
        }
        build_router(state)
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    /// The whole point of the gate. Every path that can hand back purchased art —
    /// the page, the wasm, and every file under it — has to refuse a stranger,
    /// and has to say what it wants, or the browser never offers a login box.
    #[tokio::test]
    async fn a_stranger_is_asked_for_the_password_before_any_licensed_art() {
        for uri in ["/", "/runtime-assets/interiors/office.png", "/index.html"] {
            let response = build_router(gated_state())
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{uri}");
            assert_eq!(
                response.headers()[header::WWW_AUTHENTICATE],
                r#"Basic realm="Waystation""#,
                "{uri}"
            );
        }
    }

    /// The platform's health check carries no credentials. Gating it would have
    /// the host conclude the machine was dead and take it away mid-demo.
    #[tokio::test]
    async fn the_health_check_is_never_asked_for_a_password() {
        assert_eq!(
            status_of(gated_state(), "/api/health", None).await,
            StatusCode::OK
        );
    }

    /// The wrong password has to fail even though it is the right shape, and the
    /// right one has to reach past the gate rather than merely satisfy it.
    #[tokio::test]
    async fn only_the_credential_that_was_issued_opens_the_door() {
        assert_eq!(
            status_of(gated_state(), "/", Some("judge:guess")).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status_of(gated_state(), "/", Some("stranger:let-me-in")).await,
            StatusCode::UNAUTHORIZED
        );
        // `dist/index.html` is not built during tests, so reaching the handler at
        // all is the assertion; what matters is that it is no longer a 401.
        assert_ne!(
            status_of(gated_state(), "/", Some("judge:let-me-in")).await,
            StatusCode::UNAUTHORIZED
        );
    }

    /// Local development and `make run-container` set nothing, and must stay
    /// exactly as open as they were before the gate existed.
    #[tokio::test]
    async fn an_ungated_waystation_asks_nobody_for_anything() {
        assert_eq!(
            status_of(state_with(ApiMode::Fixture, None), "/api/health", None).await,
            StatusCode::OK
        );
        assert_ne!(
            status_of(state_with(ApiMode::Fixture, None), "/", None).await,
            StatusCode::UNAUTHORIZED
        );
    }

    fn origin_of(response: &Response<Body>) -> Option<&str> {
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|value| value.to_str().ok())
    }

    async fn preflight(state: AppState, origin: &str) -> Response<Body> {
        build_router(state)
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/api/interpret")
                    .header("origin", origin)
                    .header("access-control-request-method", "POST")
                    .header("access-control-request-headers", "content-type")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    fn state_allowing(origins: &[&str]) -> AppState {
        let mut state = state_with(ApiMode::Fixture, None);
        state.config.allowed_origins = origins.iter().map(|o| (*o).to_owned()).collect();
        state
    }

    /// The submitted demo link is a static host and cannot be moved, so the page
    /// and the API sit on different origins. The browser asks first, and a
    /// server that does not answer means every traveler silently falls back to
    /// the reviewed fixture on the one link that was published.
    #[tokio::test]
    async fn the_submitted_page_is_allowed_to_reach_the_api_from_its_own_host() {
        const PAGES: &str = "https://swelljoe.github.io";

        let asked = preflight(state_allowing(&[PAGES]), PAGES).await;
        assert!(asked.status().is_success(), "{:?}", asked.status());
        assert_eq!(origin_of(&asked), Some(PAGES));

        let posted = build_router(state_allowing(&[PAGES]))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/interpret")
                    .header("origin", PAGES)
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"vignette_id":"mara_grief"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(posted.status(), StatusCode::OK);
        assert_eq!(origin_of(&posted), Some(PAGES));
    }

    /// Named origins only. A wildcard would let any page on the internet spend
    /// the Gloo quota this server pays for.
    #[tokio::test]
    async fn a_page_nobody_named_is_not_answered() {
        let state = state_allowing(&["https://swelljoe.github.io"]);
        let response = preflight(state, "https://not-ours.example").await;
        assert_eq!(origin_of(&response), None);
    }

    /// Local development names no origin and must keep behaving exactly as it
    /// did before any of this existed: no header, no preflight handling, and the
    /// page this server serves itself still works because it shares the origin.
    #[tokio::test]
    async fn an_unnamed_deployment_sends_no_cross_origin_header_at_all() {
        let state = state_with(ApiMode::Fixture, None);
        assert!(state.config.allowed_origins.is_empty());
        let response = preflight(state, "https://swelljoe.github.io").await;
        assert_eq!(origin_of(&response), None);
    }

    /// A preflight carries no credentials by design. If the gate were layered
    /// above the permission check, the browser's question would come back 401
    /// and the real request would never be sent — a gated deployment would look
    /// like a broken one rather than a locked one.
    #[tokio::test]
    async fn the_browsers_question_is_answered_before_the_door_is_consulted() {
        const PAGES: &str = "https://swelljoe.github.io";
        let mut state = state_allowing(&[PAGES]);
        state.config.gate = Credential::parse("judge:let-me-in");

        let response = preflight(state, PAGES).await;

        assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(origin_of(&response), Some(PAGES));
    }

    #[test]
    fn a_half_written_credential_is_no_credential() {
        assert!(Credential::parse("judge:let-me-in").is_some());
        assert!(Credential::parse("judge:").is_none());
        assert!(Credential::parse(":let-me-in").is_none());
        assert!(Credential::parse("judge").is_none());
        assert!(Credential::parse("").is_none());
    }
}
