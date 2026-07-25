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

static VIGNETTES: OnceLock<Vec<Vignette>> = OnceLock::new();
static PASSAGES: OnceLock<Vec<PassageCandidate>> = OnceLock::new();

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
