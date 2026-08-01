//! What turns up when you search a ruin.
//!
//! Almost none of it is useful, and that is deliberate. A player who only ever
//! searches when there is something to gain stops searching the moment the
//! rewards dry up; a player who finds a bent coat-hanger and a photograph of two
//! strangers keeps opening drawers. It also does the world-building that no
//! exposition could: the Scribe can read, and still cannot tell a television
//! remote from a religious object, because nobody alive can.
//!
//! Each search spot gives up one find and is then empty. Finds do not repeat
//! within a run until the catalogue is exhausted.

use std::sync::OnceLock;

use bevy::prelude::*;
use serde::Deserialize;

use crate::chance::Chance;
use crate::progression::SupplyId;

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Reward {
    pub item: SupplyId,
    pub amount: u16,
}

#[derive(Debug, Deserialize)]
pub struct Find {
    pub id: String,
    pub label: String,
    pub line: String,
    #[serde(default)]
    pub reward: Option<Reward>,
}

#[derive(Debug, Deserialize)]
struct Catalogue {
    finds: Vec<Find>,
}

static FINDS: OnceLock<Vec<Find>> = OnceLock::new();

pub fn finds() -> &'static [Find] {
    FINDS.get_or_init(|| {
        let catalogue: Catalogue =
            serde_json::from_str(include_str!("../../../content/salvage.json"))
                .expect("content/salvage.json must be valid");
        catalogue.finds
    })
}

/// Which finds have already come up, so a run does not hand the player the same
/// coat-hanger in four different rooms.
#[derive(Resource, Default, Debug)]
pub struct Salvaged(Vec<String>);

impl Salvaged {
    /// Draws something not yet found. Once everything has been found the
    /// catalogue reopens, because an empty drawer message is worse than a
    /// repeated one and there is no way to author enough junk for a long game.
    pub fn draw(&mut self, chance: &mut Chance) -> Option<&'static Find> {
        let fresh = finds()
            .iter()
            .filter(|find| !self.0.contains(&find.id))
            .collect::<Vec<_>>();
        let found = if fresh.is_empty() {
            self.0.clear();
            chance.pick(finds())?
        } else {
            *chance.pick(&fresh)?
        };
        self.0.push(found.id.clone());
        Some(found)
    }

    #[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
    pub fn restore(&mut self, seen: Vec<String>) {
        self.0 = seen;
    }

    pub fn seen(&self) -> &[String] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogue_parses_and_every_find_says_something() {
        let finds = finds();
        assert!(finds.len() >= 12, "a drawer needs plenty to be hiding");
        for find in finds {
            assert!(!find.label.trim().is_empty(), "{} has no label", find.id);
            assert!(
                find.line.split_whitespace().count() >= 8,
                "{} is too terse to be worth finding",
                find.id
            );
        }
    }

    #[test]
    fn most_of_what_the_ruins_hold_is_worth_nothing() {
        let useful = finds().iter().filter(|find| find.reward.is_some()).count();
        let total = finds().len();
        assert!(
            useful * 3 <= total,
            "{useful} of {total} finds pay out; the land is being too generous"
        );
    }

    #[test]
    fn find_ids_are_unique() {
        let mut ids = finds().iter().map(|find| find.id.as_str()).collect::<Vec<_>>();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(count, ids.len(), "duplicate salvage id");
    }

    #[test]
    fn a_run_works_through_the_catalogue_before_repeating_itself() {
        let mut salvaged = Salvaged::default();
        let mut chance = Chance::default();
        let mut drawn = Vec::new();
        for _ in 0..finds().len() {
            let find = salvaged.draw(&mut chance).expect("a find");
            assert!(!drawn.contains(&find.id), "{} came up twice", find.id);
            drawn.push(find.id.clone());
        }
        assert!(
            salvaged.draw(&mut chance).is_some(),
            "an exhausted catalogue should reopen rather than go silent"
        );
    }

    #[test]
    fn a_restored_save_does_not_re_offer_what_was_already_found() {
        let mut salvaged = Salvaged::default();
        let already = finds()
            .iter()
            .take(finds().len() - 1)
            .map(|find| find.id.clone())
            .collect::<Vec<_>>();
        let last = finds().last().expect("a catalogue").id.clone();
        salvaged.restore(already);
        let mut chance = Chance::default();
        assert_eq!(salvaged.draw(&mut chance).expect("a find").id, last);
    }
}
