//! The prints the Scribe cuts at night.
//!
//! Nobody asks for these and nothing is unlocked by making them. They are what
//! the Scribe does with their hands after dark, when the day's work is finished
//! and the alternative is lying in a strange room thinking about the people who
//! are not there. The images come out of whatever has been read that day; the
//! words are copied exactly, because getting them wrong would be the one
//! unforgivable thing.
//!
//! They only become a gift later, when somebody is standing in the doorway about
//! to walk back into the ash and the Scribe finds they want them to have
//! something.

use std::fmt::Write as _;
use std::sync::OnceLock;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::chance::Chance;

/// How much colour the Scribe can get onto a block. Every stage past the first
/// waits on dyes, and dyes come from people, which is the point.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    #[default]
    Monochrome,
    TwoColour,
    FullColour,
}

impl Tier {
    const fn label(self) -> &'static str {
        match self {
            Self::Monochrome => "black ink only",
            Self::TwoColour => "black and one colour",
            Self::FullColour => "a full register of colour",
        }
    }

    fn parse(stage: &str) -> Self {
        match stage {
            "two_colour" | "spot_colour" => Self::TwoColour,
            "full_colour" => Self::FullColour,
            _ => Self::Monochrome,
        }
    }
}

#[derive(Debug, Deserialize)]
struct PrintDefinition {
    id: String,
    title: String,
    theme: String,
    reference: String,
    verse: String,
    #[serde(default)]
    stage: String,
}

#[derive(Debug, Deserialize)]
struct Catalogue {
    prints: Vec<PrintDefinition>,
}

#[derive(Debug)]
pub struct Print {
    pub id: String,
    pub title: String,
    pub theme: String,
    pub reference: String,
    pub verse: String,
    pub tier: Tier,
}

impl Print {
    /// Where the composed card lives in the runtime tree.
    pub fn art_path(&self) -> String {
        format!("prints/{}-card.png", self.id)
    }
}

static PRINTS: OnceLock<Vec<Print>> = OnceLock::new();

pub fn prints() -> &'static [Print] {
    PRINTS.get_or_init(|| {
        let catalogue: Catalogue =
            serde_json::from_str(include_str!("../../../content/prints.json"))
                .expect("content/prints.json must be valid");
        catalogue
            .prints
            .into_iter()
            .map(|definition| Print {
                tier: Tier::parse(&definition.stage),
                id: definition.id,
                title: definition.title,
                theme: definition.theme,
                reference: definition.reference,
                verse: definition.verse,
            })
            .collect()
    })
}

pub fn print(id: &str) -> Option<&'static Print> {
    prints().iter().find(|print| print.id == id)
}

/// What a traveller's need has to do with what is cut on a block. The Scribe is
/// not matching a key to a lock — several themes would answer most needs — but
/// something has to be reached for first, and this is what they reach for.
const NEED_THEMES: [(&str, &str); 6] = [
    ("comfort", "gentleness"),
    ("presence", "hope"),
    ("rest", "rest"),
    ("courage", "hope"),
    ("belonging", "hospitality"),
    ("mercy", "mutual_aid"),
];

#[derive(Resource, Default, Debug)]
pub struct Collection {
    /// Blocks cut, oldest first.
    made: Vec<String>,
    /// Blocks that have gone out into the wastes in somebody's coat.
    given: Vec<String>,
    /// The best colour the Scribe has learned to lay down.
    pub tier: Tier,
}

impl Collection {
    pub fn made(&self) -> &[String] {
        &self.made
    }

    pub const fn given_count(&self) -> usize {
        self.given.len()
    }

    pub fn has(&self, id: &str) -> bool {
        self.made.iter().any(|made| made == id)
    }

    /// Whether this one left with somebody. It stays in `made` either way: the
    /// folio is a record of what the Scribe has cut, not of what is in reach.
    pub fn was_given(&self, id: &str) -> bool {
        self.given.iter().any(|given| given == id)
    }

    /// Everything cut and not yet given away, in the order it was made.
    pub fn on_hand(&self) -> Vec<&'static Print> {
        self.made
            .iter()
            .filter(|id| !self.given.contains(id))
            .filter_map(|id| print(id))
            .collect()
    }

    /// A night's work. What was read that day is what the hands reach for, so a
    /// player who has been reading about hospitality tends to wake up with a
    /// hospitality card. Returns `None` when the Scribe has already cut
    /// everything they know how to cut in the colours they have — which is a
    /// quiet, findable reason to want dyes.
    pub fn cut_a_block(
        &mut self,
        chance: &mut Chance,
        dwelling_on: Option<&str>,
    ) -> Option<&'static Print> {
        let available = prints()
            .iter()
            .filter(|print| print.tier <= self.tier && !self.has(&print.id))
            .collect::<Vec<_>>();
        let on_the_mind = dwelling_on.map_or_else(Vec::new, |theme| {
            available
                .iter()
                .copied()
                .filter(|print| print.theme == theme)
                .collect()
        });
        let chosen = if on_the_mind.is_empty() {
            *chance.pick(&available)?
        } else {
            *chance.pick(&on_the_mind)?
        };
        self.made.push(chosen.id.clone());
        Some(chosen)
    }

    pub fn give(&mut self, id: &str) {
        if self.has(id) && !self.given.iter().any(|given| given == id) {
            self.given.push(id.to_owned());
        }
    }

    /// Which card on hand the Scribe reaches for, given what they heard. The
    /// player is free to hand over a different one; nothing is scored.
    pub fn suggestion_for(&self, need_id: &str) -> Option<&'static Print> {
        let theme = NEED_THEMES
            .iter()
            .find(|(need, _)| *need == need_id)
            .map(|(_, theme)| *theme)?;
        self.on_hand()
            .into_iter()
            .find(|print| print.theme == theme)
    }

    /// What the Scribe can currently manage, for the corner of the screen.
    pub fn describe(&self) -> String {
        let on_hand = self.on_hand().len();
        let given = self.given_count();
        let mut line = format!("{on_hand} cut and kept · {}", self.tier.label());
        if given > 0 {
            let word = if given == 1 { "card" } else { "cards" };
            let _ = write!(line, "\n{given} {word} carried away");
        }
        line
    }

    /// Restores a saved collection. Unknown ids are dropped rather than kept,
    /// so removing a print from the catalogue cannot strand a save.
    #[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
    pub fn restore(&mut self, made: Vec<String>, given: Vec<String>, tier: Tier) {
        self.made = made.into_iter().filter(|id| print(id).is_some()).collect();
        self.given = given
            .into_iter()
            .filter(|id| self.made.contains(id))
            .collect();
        self.tier = tier;
    }

    pub fn saved(&self) -> (Vec<String>, Vec<String>, Tier) {
        (self.made.clone(), self.given.clone(), self.tier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogue_parses_and_every_print_carries_its_words() {
        let prints = prints();
        assert!(prints.len() >= 5, "the first set is five cards");
        for print in prints {
            assert!(!print.verse.trim().is_empty(), "{} has no verse", print.id);
            assert!(
                !print.reference.trim().is_empty(),
                "{} has no reference",
                print.id
            );
            assert!(
                print.art_path().starts_with("prints/"),
                "{} points outside the runtime print tree",
                print.id
            );
        }
    }

    #[test]
    fn print_ids_are_unique_so_a_card_cannot_be_cut_twice() {
        let mut ids = prints()
            .iter()
            .map(|print| print.id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(count, ids.len(), "duplicate print id in the catalogue");
    }

    #[test]
    fn nights_of_cutting_never_repeat_a_block_and_stop_when_the_set_is_done() {
        let mut collection = Collection::default();
        let mut chance = Chance::default();
        let monochrome = prints()
            .iter()
            .filter(|print| print.tier == Tier::Monochrome)
            .count();
        let mut cut = Vec::new();
        for _ in 0..monochrome {
            let print = collection
                .cut_a_block(&mut chance, None)
                .expect("a block to cut");
            assert!(!cut.contains(&print.id), "cut {} twice", print.id);
            cut.push(print.id.clone());
        }
        assert!(
            collection.cut_a_block(&mut chance, None).is_none(),
            "the Scribe kept inventing cards they have no colour for"
        );
    }

    #[test]
    fn colour_gates_the_later_cards_rather_than_hiding_them() {
        let coloured = prints()
            .iter()
            .filter(|print| print.tier > Tier::Monochrome)
            .count();
        if coloured == 0 {
            return;
        }
        let mut collection = Collection::default();
        let mut chance = Chance::default();
        while collection.cut_a_block(&mut chance, None).is_some() {}
        let before = collection.made().len();
        collection.tier = Tier::FullColour;
        assert!(
            collection.cut_a_block(&mut chance, None).is_some(),
            "learning dyes should open the blocks that need them"
        );
        assert_eq!(collection.made().len(), before + 1);
    }

    #[test]
    fn a_given_card_leaves_the_hand_but_stays_in_the_history() {
        let mut collection = Collection::default();
        let mut chance = Chance::default();
        let id = collection
            .cut_a_block(&mut chance, None)
            .expect("a block")
            .id
            .clone();
        assert_eq!(collection.on_hand().len(), 1);
        collection.give(&id);
        assert!(collection.on_hand().is_empty());
        assert!(collection.has(&id), "the Scribe remembers cutting it");
        collection.give(&id);
        assert_eq!(collection.given_count(), 1, "given away twice");
    }

    #[test]
    fn a_card_that_was_never_cut_cannot_be_given() {
        let mut collection = Collection::default();
        collection.give("early-hospitality");
        assert_eq!(collection.given_count(), 0);
    }

    #[test]
    fn every_authored_need_reaches_for_a_theme_that_exists_in_the_catalogue() {
        for (need, theme) in NEED_THEMES {
            assert!(
                prints().iter().any(|print| print.theme == theme),
                "need {need} reaches for theme {theme}, which no card carries"
            );
        }
    }

    #[test]
    fn a_suggestion_only_ever_names_a_card_actually_in_hand() {
        let mut collection = Collection::default();
        assert!(
            collection.suggestion_for("belonging").is_none(),
            "an empty hand cannot suggest anything"
        );
        collection.restore(
            vec!["early-hospitality".to_owned()],
            Vec::new(),
            Tier::Monochrome,
        );
        let suggestion = collection
            .suggestion_for("belonging")
            .expect("a suggestion");
        assert_eq!(suggestion.id, "early-hospitality");
        assert!(
            collection.suggestion_for("no-such-need").is_none(),
            "an unknown need must not fall back to an arbitrary card"
        );
        collection.give("early-hospitality");
        assert!(
            collection.suggestion_for("belonging").is_none(),
            "a card already carried away was suggested again"
        );
    }

    #[test]
    fn restoring_a_save_drops_prints_the_catalogue_no_longer_has() {
        let mut collection = Collection::default();
        collection.restore(
            vec!["early-rest".to_owned(), "a-card-that-was-cut".to_owned()],
            vec!["a-card-that-was-cut".to_owned()],
            Tier::TwoColour,
        );
        assert_eq!(collection.made(), ["early-rest"]);
        assert_eq!(collection.given_count(), 0);
        assert_eq!(collection.tier, Tier::TwoColour);
    }
}
