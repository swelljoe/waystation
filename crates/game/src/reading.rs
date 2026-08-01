//! The little book on the nightstand.
//!
//! It is not an inventory item and it never leaves room three. The Scribe opens
//! it, reads whatever it falls open at, sits with it, and puts it back — which
//! is both how a person actually reads and how the game avoids turning Scripture
//! into a collectible.
//!
//! The first reading is fixed. It has to be: the whole idea of feeding a
//! stranger has to get into the Scribe's head *before* a stranger exists, or the
//! welcome is just a quest step. Everything after that is drawn at random, and
//! what was read most recently is what the hands reach for at night when there
//! is a blank block and nothing else to do.

use std::sync::OnceLock;

use bevy::prelude::*;
use serde::Deserialize;

use crate::chance::Chance;

#[derive(Debug, Deserialize)]
pub struct Reading {
    pub id: String,
    pub reference: String,
    pub verse: String,
    pub theme: String,
    pub reflection: String,
    /// The one the book always falls open at the first time.
    #[serde(default)]
    pub first: bool,
}

#[derive(Debug, Deserialize)]
struct Catalogue {
    readings: Vec<Reading>,
}

static READINGS: OnceLock<Vec<Reading>> = OnceLock::new();

pub fn readings() -> &'static [Reading] {
    READINGS.get_or_init(|| {
        let catalogue: Catalogue =
            serde_json::from_str(include_str!("../../../content/readings.json"))
                .expect("content/readings.json must be valid");
        catalogue.readings
    })
}

#[derive(Resource, Default, Debug)]
pub struct Readings {
    read: Vec<String>,
    /// The theme of the last passage read, which is what the night's block is
    /// most likely to carry.
    pub dwelling_on: Option<String>,
}

impl Readings {
    pub const fn has_read_anything(&self) -> bool {
        !self.read.is_empty()
    }

    pub const fn count(&self) -> usize {
        self.read.len()
    }

    /// Opens the book. The first time this is the authored opening passage;
    /// after that it is whatever has not been read lately.
    pub fn open(&mut self, chance: &mut Chance) -> Option<&'static Reading> {
        let chosen = if self.read.is_empty() {
            readings()
                .iter()
                .find(|reading| reading.first)
                .or_else(|| readings().first())?
        } else {
            let unread = readings()
                .iter()
                .filter(|reading| !self.read.contains(&reading.id))
                .collect::<Vec<_>>();
            if unread.is_empty() {
                // A book you have finished is a book you start again. Keep the
                // most recent reading out of the draw so it cannot repeat twice
                // in a row, which reads as a bug even when it is not.
                let last = self.read.last().cloned().unwrap_or_default();
                let rest = readings()
                    .iter()
                    .filter(|reading| reading.id != last)
                    .collect::<Vec<_>>();
                *chance.pick(&rest)?
            } else {
                *chance.pick(&unread)?
            }
        };
        self.read.push(chosen.id.clone());
        self.dwelling_on = Some(chosen.theme.clone());
        Some(chosen)
    }

    #[allow(dead_code)]
    pub fn restore(&mut self, read: Vec<String>, dwelling_on: Option<String>) {
        self.read = read;
        self.dwelling_on = dwelling_on;
    }

    pub fn saved(&self) -> (Vec<String>, Option<String>) {
        (self.read.clone(), self.dwelling_on.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogue_parses_and_carries_words_and_a_reaction() {
        let readings = readings();
        assert!(readings.len() >= 10);
        for reading in readings {
            assert!(!reading.verse.trim().is_empty(), "{} has no verse", reading.id);
            assert!(
                !reading.reference.trim().is_empty(),
                "{} has no reference",
                reading.id
            );
            assert!(
                !reading.reflection.trim().is_empty(),
                "{} has no reaction; a passage the Scribe does not respond to is a lecture",
                reading.id
            );
        }
    }

    #[test]
    fn exactly_one_passage_is_the_one_the_book_opens_at() {
        let first = readings().iter().filter(|reading| reading.first).count();
        assert_eq!(first, 1, "the opening reading must be unambiguous");
    }

    #[test]
    fn the_first_reading_is_about_taking_a_stranger_in() {
        let mut readings_state = Readings::default();
        let mut chance = Chance::default();
        let first = readings_state.open(&mut chance).expect("a reading");
        assert_eq!(
            first.theme, "hospitality",
            "the idea has to arrive before anyone does"
        );
    }

    #[test]
    fn the_book_works_through_itself_before_it_starts_again() {
        let mut state = Readings::default();
        let mut chance = Chance::default();
        let mut seen = Vec::new();
        for _ in 0..readings().len() {
            let reading = state.open(&mut chance).expect("a reading");
            assert!(!seen.contains(&reading.id), "{} came up twice", reading.id);
            seen.push(reading.id.clone());
        }
        let again = state.open(&mut chance).expect("a reading");
        assert_ne!(
            Some(&again.id),
            seen.last(),
            "the book fell open at the same page twice running"
        );
    }

    #[test]
    fn reading_leaves_the_scribe_dwelling_on_that_theme() {
        let mut state = Readings::default();
        let mut chance = Chance::default();
        assert!(state.dwelling_on.is_none());
        let reading = state.open(&mut chance).expect("a reading");
        assert_eq!(state.dwelling_on.as_deref(), Some(reading.theme.as_str()));
        assert_eq!(state.count(), 1);
        assert!(state.has_read_anything());
    }

    #[test]
    fn every_reading_theme_is_one_a_block_can_actually_carry() {
        for reading in readings() {
            assert!(
                crate::cards::prints()
                    .iter()
                    .any(|print| print.theme == reading.theme),
                "{} is read but no print carries theme {}",
                reading.id,
                reading.theme
            );
        }
    }
}
