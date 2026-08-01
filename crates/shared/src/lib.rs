//! Shared, platform-neutral contracts for the Waystation game and API service.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

pub const DEFAULT_BIBLE_ID: u32 = 3034;
pub const DEFAULT_BIBLE_ABBREVIATION: &str = "BSB";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub status: String,
    pub api_mode: String,
    pub gloo_configured: bool,
    pub youversion_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterpretRequest {
    pub vignette_id: String,
    /// The player's language, as a BCP-47 tag the client already knows about
    /// itself. Absent means English. It selects the translation of the passage
    /// and nothing else — the vignette and the reflection around it stay in the
    /// language they were authored in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterpretResponse {
    pub vignette_id: String,
    pub need_id: String,
    pub need_label: String,
    pub reflection: String,
    pub passage: Passage,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Passage {
    pub id: String,
    pub reference: String,
    pub content: String,
    pub version: String,
    pub youversion_deep_link: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Provenance {
    pub gloo_model: String,
    pub routing: String,
    pub scripture_source: ScriptureSource,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScriptureSource {
    YouVersionLive,
    /// Gloo chose the passage, but no `YouVersion` key was configured, so the text
    /// is the reviewed wording already in `content/passages.ron`. The selection
    /// is live; the words are the ones a human already approved.
    ReviewedLocal,
    Fixture,
    Cache,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Vignette {
    pub id: String,
    pub traveler_name: String,
    pub lines: Vec<String>,
    pub needs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PassageCandidate {
    pub id: String,
    pub need_id: String,
    pub need_label: String,
    pub reference: String,
    pub fixture_content: String,
}

/// One translation, chosen for one language. Built by
/// `scripts/fetch-bible-versions.py` and then reviewed; the runtime only reads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BibleVersion {
    pub id: u32,
    pub abbreviation: String,
    pub title: String,
    pub language: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogEntry {
    #[serde(flatten)]
    chosen: BibleVersion,
    #[serde(default)]
    alternatives: Vec<BibleVersion>,
}

#[derive(Debug, Clone, Deserialize)]
struct BibleCatalog {
    languages: Vec<CatalogEntry>,
}

static VIGNETTES: OnceLock<Vec<Vignette>> = OnceLock::new();
static PASSAGES: OnceLock<Vec<PassageCandidate>> = OnceLock::new();
static BIBLE_VERSIONS: OnceLock<Vec<BibleVersion>> = OnceLock::new();

/// Every translation the catalog knows, each language's chosen pick first.
///
/// Picks and alternatives share one list so that `version_by_id` can find any
/// catalogued edition, while `version_for_language` — which takes the first
/// match — still lands on the reviewed pick.
#[must_use]
pub fn bible_versions() -> &'static [BibleVersion] {
    BIBLE_VERSIONS.get_or_init(|| {
        serde_json::from_str::<BibleCatalog>(include_str!("../../../content/bible-versions.json"))
            .expect("content/bible-versions.json must be valid")
            .languages
            .into_iter()
            .flat_map(|entry| std::iter::once(entry.chosen).chain(entry.alternatives))
            .collect()
    })
}

/// The translation for a player's language tag, most specific match first.
///
/// The tag is tried whole, then with one trailing subtag dropped at a time, down
/// to the primary. `zh-Hant-TW` finds the Traditional Chinese edition the
/// catalog lists under exactly that tag; `pt-BR`, which the catalog does not
/// distinguish, falls through to Portuguese. Going straight to the primary
/// subtag would have handed every Chinese player the Simplified text, and
/// stopping at the whole tag would have handed a Brazilian player nothing.
#[must_use]
pub fn version_for_language(tag: &str) -> Option<&'static BibleVersion> {
    let normalized = tag.replace('_', "-").to_ascii_lowercase();
    let mut candidate = normalized.as_str();
    loop {
        if !candidate.is_empty() {
            if let Some(version) = bible_versions()
                .iter()
                .find(|version| version.language.eq_ignore_ascii_case(candidate))
            {
                return Some(version);
            }
        }
        candidate = &candidate[..candidate.rfind('-')?];
    }
}

#[must_use]
pub fn version_by_id(id: u32) -> Option<&'static BibleVersion> {
    bible_versions().iter().find(|version| version.id == id)
}

/// What a player gets when their language is absent, unrecognised, or has no
/// translation carrying all the books the authored passages draw from.
///
/// # Panics
///
/// If the catalog has lost its English entry, which the generator refuses to
/// produce; there would be nothing left to serve.
#[must_use]
pub fn fallback_version() -> &'static BibleVersion {
    bible_versions()
        .iter()
        .find(|version| version.abbreviation == DEFAULT_BIBLE_ABBREVIATION)
        .expect("content/bible-versions.json must keep the English fallback")
}

#[must_use]
pub fn vignettes() -> &'static [Vignette] {
    VIGNETTES.get_or_init(|| {
        ron::from_str(include_str!("../../../content/vignettes.ron"))
            .expect("content/vignettes.ron must be valid")
    })
}

#[must_use]
pub fn passages() -> &'static [PassageCandidate] {
    PASSAGES.get_or_init(|| {
        ron::from_str(include_str!("../../../content/passages.ron"))
            .expect("content/passages.ron must be valid")
    })
}

#[must_use]
pub fn vignette(id: &str) -> Option<&'static Vignette> {
    vignettes().iter().find(|item| item.id == id)
}

#[must_use]
pub fn passage(id: &str) -> Option<&'static PassageCandidate> {
    passages().iter().find(|item| item.id == id)
}

#[must_use]
pub fn candidates_for(vignette: &Vignette) -> Vec<&'static PassageCandidate> {
    passages()
        .iter()
        .filter(|candidate| vignette.needs.contains(&candidate.need_id))
        .collect()
}

#[must_use]
pub fn valid_selection(vignette: &Vignette, need_id: &str, passage_id: &str) -> bool {
    vignette.needs.iter().any(|need| need == need_id)
        && passage(passage_id).is_some_and(|candidate| candidate.need_id == need_id)
}

#[must_use]
pub fn fixture_response(vignette_id: &str) -> Option<InterpretResponse> {
    let vignette = vignette(vignette_id)?;
    let candidate = candidates_for(vignette).into_iter().next()?;
    Some(InterpretResponse {
        vignette_id: vignette.id.clone(),
        need_id: candidate.need_id.clone(),
        need_label: candidate.need_label.clone(),
        reflection: fixture_reflection(&candidate.need_id).to_owned(),
        passage: Passage {
            id: candidate.id.clone(),
            reference: candidate.reference.clone(),
            content: candidate.fixture_content.clone(),
            version: DEFAULT_BIBLE_ABBREVIATION.to_owned(),
            youversion_deep_link: format!(
                "https://www.bible.com/bible/{DEFAULT_BIBLE_ID}/{}",
                candidate.id
            ),
        },
        provenance: Provenance {
            gloo_model: "fixture-reviewed-v1".to_owned(),
            routing: "fixture".to_owned(),
            scripture_source: ScriptureSource::Fixture,
        },
    })
}

#[must_use]
pub fn fixture_reflection(need_id: &str) -> &'static str {
    match need_id {
        "comfort" => "Some grief cannot be carried away, but it need not be carried alone.",
        "presence" => "A dark road is changed when someone promises to walk it beside us.",
        "rest" => {
            "Rest is not the abandonment of your burden; it is how you become able to bear it."
        }
        "courage" => "Strength can return quietly, one breath and one faithful step at a time.",
        "belonging" => {
            "No living soul is extra. A name spoken with welcome becomes part of the shelter."
        }
        "mercy" => "Mercy makes room before it asks whether the traveler has earned a place.",
        _ => "There are old words here for what you carry.",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CardRecipe {
    pub paper: u8,
    pub illustration: u8,
    pub border: u8,
}

impl Default for CardRecipe {
    fn default() -> Self {
        Self {
            paper: 1,
            illustration: 1,
            border: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_vignette_has_candidates() {
        for item in vignettes() {
            assert!(candidates_for(item).len() >= 2);
        }
    }

    #[test]
    fn selection_must_match_vignette_and_need() {
        let mara = vignette("mara_grief").unwrap();
        assert!(valid_selection(mara, "comfort", "PSA.34.18"));
        assert!(!valid_selection(mara, "rest", "MAT.11.28-30"));
        assert!(!valid_selection(mara, "comfort", "LUK.6.36"));
    }

    /// A version that lacks a book an authored passage draws from would answer
    /// a Psalm with a 404 in the player's own language. The generator filters
    /// on this; the check belongs here too, because the file is hand-editable.
    #[test]
    fn every_offered_translation_carries_every_authored_book() {
        let catalog: serde_json::Value =
            serde_json::from_str(include_str!("../../../content/bible-versions.json")).unwrap();
        let required: Vec<&str> = catalog["required_books"]
            .as_array()
            .unwrap()
            .iter()
            .map(|book| book.as_str().unwrap())
            .collect();
        let used: std::collections::BTreeSet<&str> = passages()
            .iter()
            .map(|candidate| &candidate.id[..3])
            .collect();

        for book in used {
            assert!(
                required.contains(&book),
                "content/passages.ron draws from {book}, which no translation was filtered on; \
                 rerun `make bible-versions`"
            );
        }
        assert!(
            bible_versions().len() > 1,
            "the catalog collapsed to English"
        );
    }

    /// A regional tag the catalog does not distinguish falls through to its
    /// language; a Brazilian player should be handed Portuguese, not English.
    #[test]
    fn a_regional_tag_still_finds_its_language() {
        let plain = version_for_language("pt").expect("Portuguese is mapped");
        for regional in ["pt-BR", "pt_BR", "PT-br"] {
            assert_eq!(version_for_language(regional), Some(plain), "{regional}");
        }
        assert_eq!(version_for_language(""), None);
        assert_eq!(version_for_language("zzz"), None);
    }

    /// Where the catalog *does* distinguish a script, the specific entry has to
    /// win. Matching on the primary subtag alone would hand every Chinese
    /// player the Simplified text, Taiwan included.
    #[test]
    fn a_script_specific_tag_beats_its_bare_language() {
        let traditional =
            version_for_language("zh-Hant-TW").expect("Traditional Chinese is mapped");
        let simplified = version_for_language("zh").expect("Chinese is mapped");

        assert_eq!(traditional.language, "zh-Hant-TW");
        assert_ne!(traditional.id, simplified.id);
        assert_eq!(version_for_language("ZH_HANT_TW"), Some(traditional));
        // An unlisted region under a listed script still reaches Chinese.
        assert_eq!(version_for_language("zh-CN"), Some(simplified));
    }

    /// Everything that cannot be translated has to land somewhere, and that
    /// somewhere is the English the reflections were authored around.
    #[test]
    fn the_fallback_is_the_english_the_fixtures_were_written_in() {
        let fallback = fallback_version();
        assert_eq!(fallback.abbreviation, DEFAULT_BIBLE_ABBREVIATION);
        assert_eq!(fallback.language, "en");
        assert_eq!(fallback.id, DEFAULT_BIBLE_ID);
        assert_eq!(version_by_id(DEFAULT_BIBLE_ID), Some(fallback));
    }

    #[test]
    fn fixture_responses_are_traceable() {
        let response = fixture_response("fen_belonging").unwrap();
        assert_eq!(response.passage.id, "GAL.3.28");
        assert_eq!(
            response.provenance.scripture_source,
            ScriptureSource::Fixture
        );
    }
}
