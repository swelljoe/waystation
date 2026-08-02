//! Standing a visit up on demand, for looking at it.
//!
//! Arrivals are rare on purpose. A keeper who lights the fire on their first
//! evening still waits three nights before anyone risks the walk down, and then
//! the dice decide, and then the hour does. That pace is right for playing and
//! useless for writing: nobody can iterate on what a stranger says when it takes
//! ten minutes of real time to hear one and another ten to hear the next.
//!
//! So this is a switch that is off unless it is asked for by name. When it is
//! on, somebody is already coming down the road as the game opens, the
//! waystation is warm enough to actually offer them something, and F4 fetches
//! the next one whenever the court is empty. Nothing here is reachable from
//! inside the game: no key combination turns it on, and a player who never sets
//! the variable can never trip over it.
//!
//! ```text
//! WAYSTATION_VISITORS=now                    cargo run -p waystation-game
//! WAYSTATION_VISITORS=repeat                 cargo run -p waystation-game
//! WAYSTATION_VISITORS=story=sela_offer       cargo run -p waystation-game
//! WAYSTATION_VISITORS=who=old-hand,repeat    cargo run -p waystation-game
//! WAYSTATION_VISITORS=now,cold               cargo run -p waystation-game
//! ```
//!
//! On the web the same spec is a query parameter: `index.html?visitors=repeat`.

use bevy::prelude::*;

use crate::cards::Collection;
use crate::chance::Chance;
use crate::daylight::Clock;
use crate::progression::{Progression, SupplyId};
use crate::visitors::{Visitors, Wanted, PROFILES};
use crate::{InteriorState, MotelAccess, OFFICE_CHIMNEY_STATE_KEY, OFFICE_HEARTH_STATE_KEY};

/// The environment variable, and the web query parameter, that turn this on.
pub const SWITCH: &str = "WAYSTATION_VISITORS";

/// How long the court stays empty between rehearsed visits. Long enough to read
/// the farewell, short enough that waiting is not the experience being tested.
const BEAT_BETWEEN_VISITS: f32 = 3.0;

/// Rations, keys, and cut blocks a rehearsed waystation starts with. Enough to
/// exercise every branch of the hospitality screen twice over.
const STARTING_RATIONS: u16 = 6;
const STARTING_BLOCKS: usize = 4;

/// Nights of smoke a rehearsed waystation claims to have kept. Only the arrival
/// odds read it, which rehearsal overrides anyway; it is set so the ledger does
/// not contradict a court full of visitors.
const NIGHTS_ALREADY_KEPT: u32 = 12;

/// What the switch was set to. Absent from an ordinary game.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct Rehearsal {
    /// Nothing below matters unless this is set.
    pub on: bool,
    /// Fetch another one every time the court empties, rather than one and done.
    pub repeat: bool,
    /// Always tell this story, and pick a party that could plausibly tell it.
    pub story: Option<String>,
    /// Always arrive in this shape, whatever the story.
    pub who: Option<String>,
    /// Leave the waystation as a real first day leaves it: cold hearth, no keys,
    /// nothing to give. The visit still happens; there is just nothing to offer,
    /// which is its own thing worth looking at.
    pub cold: bool,
}

impl Rehearsal {
    /// Reads the spec written in the switch.
    ///
    /// Every token is optional and any token implies `on`, so the shortest
    /// useful spec is a bare `now`. Unknown tokens are refused rather than
    /// ignored: a misspelt story id that silently rehearsed a random story
    /// would waste more time than it saved.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let mut wanted = Self::default();
        for token in spec.split([',', ' ']).filter(|token| !token.is_empty()) {
            let (key, value) = token
                .split_once('=')
                .map_or((token, None), |(key, value)| (key, Some(value)));
            match (key.trim(), value.map(str::trim)) {
                ("off" | "0" | "no" | "false", None) => return Ok(Self::default()),
                ("on" | "1" | "yes" | "now", None) => wanted.on = true,
                ("repeat", None) => {
                    wanted.on = true;
                    wanted.repeat = true;
                }
                ("cold", None) => {
                    wanted.on = true;
                    wanted.cold = true;
                }
                ("story", Some(id)) => {
                    if waystation_shared::vignette(id).is_none() {
                        return Err(format!("no story called {id:?}. Authored: {}", story_ids()));
                    }
                    wanted.on = true;
                    wanted.story = Some(id.to_owned());
                }
                ("who", Some(id)) => {
                    if !PROFILES.iter().any(|profile| profile.id == id) {
                        return Err(format!(
                            "nobody arrives as {id:?}. Shapes: {}",
                            profile_ids()
                        ));
                    }
                    wanted.on = true;
                    wanted.who = Some(id.to_owned());
                }
                _ => {
                    return Err(format!(
                        "{token:?} means nothing here. Try: now, repeat, cold, \
                         story=<id>, who=<id>, off"
                    ))
                }
            }
        }
        Ok(wanted)
    }

    /// The switch as the operating system, or the page's address, has it.
    ///
    /// A spec that does not parse stops the game rather than starting one that
    /// quietly ignores it, because the whole point of setting it was to look at
    /// something specific.
    ///
    /// # Panics
    ///
    /// If the spec is not one this understands. The message names what is valid.
    #[must_use]
    pub fn from_environment() -> Self {
        let Some(spec) = switch_setting() else {
            return Self::default();
        };
        match Self::parse(&spec) {
            Ok(wanted) => wanted,
            Err(complaint) => panic!("{SWITCH}={spec:?}: {complaint}"),
        }
    }

    /// Which story and shape an arrival should be forced into.
    ///
    /// A pinned story picks its own party when none was named, because the words
    /// have to fit the body: the one about being spoken to like a grown man
    /// belongs to the pair of children and reads as nonsense from an elder.
    #[must_use]
    pub fn wanted(&self) -> Wanted {
        let named = self.who.as_deref().and_then(profile_index);
        let profile = named.or_else(|| self.story.as_deref().and_then(first_profile_that_tells));
        Wanted {
            profile,
            vignette: self.story.clone(),
        }
    }
}

/// How far a rehearsal has got, so `repeat` can tell an empty court from a court
/// that has not been filled yet.
#[derive(Resource, Debug, Default)]
pub struct Rehearsed {
    /// Parties summoned so far.
    pub summoned: u32,
    /// Seconds the court has been empty since the last one left.
    empty_for: f32,
}

fn story_ids() -> String {
    waystation_shared::vignettes()
        .iter()
        .map(|story| story.id.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn profile_ids() -> String {
    PROFILES
        .iter()
        .map(|profile| profile.id)
        .collect::<Vec<_>>()
        .join(", ")
}

fn profile_index(id: &str) -> Option<usize> {
    PROFILES.iter().position(|profile| profile.id == id)
}

/// The first shape of party whose list includes this story. Profiles are ordered
/// most ordinary first, so a story two of them can tell lands on the commoner.
fn first_profile_that_tells(story: &str) -> Option<usize> {
    PROFILES
        .iter()
        .position(|profile| profile.vignettes.contains(&story))
}

#[cfg(not(target_arch = "wasm32"))]
fn switch_setting() -> Option<String> {
    std::env::var(SWITCH).ok()
}

/// The `visitors=` parameter of the page's own address.
///
/// Hand-parsed rather than pulled through a URL crate: it is one parameter read
/// once at startup, and the web build carries enough already.
#[cfg(target_arch = "wasm32")]
fn switch_setting() -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    search
        .trim_start_matches('?')
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| *key == "visitors")
        .map(|(_, value)| value.replace("%2C", ",").replace('+', " "))
}

/// Puts the waystation into the state a visit assumes: a fire that has been
/// kept, doors that open, food in the pack, and blocks cut and waiting.
///
/// Without this a rehearsed visit natively reaches the hospitality screen with
/// every option refused, because the native build has no save and every game
/// starts on a cold first morning.
#[allow(clippy::too_many_arguments)]
pub fn warm_the_waystation(
    rehearsal: Res<Rehearsal>,
    mut interior_state: ResMut<InteriorState>,
    mut motel_access: ResMut<MotelAccess>,
    mut progression: ResMut<Progression>,
    mut collection: ResMut<Collection>,
    mut visitors: ResMut<Visitors>,
    mut chance: ResMut<Chance>,
) {
    if !rehearsal.on {
        return;
    }
    if !rehearsal.cold {
        interior_state
            .0
            .insert(OFFICE_CHIMNEY_STATE_KEY.to_owned(), "repaired".to_owned());
        interior_state
            .0
            .insert(OFFICE_HEARTH_STATE_KEY.to_owned(), "repaired".to_owned());
        motel_access.keys_found = true;
        progression.add_supply(SupplyId::Ration, STARTING_RATIONS);
        // The tier is left where a real game leaves it. Colour waits on dyes,
        // dyes come from travellers, and rehearsing a visit is not a reason to
        // hand the Scribe a workshop they have not earned.
        for _ in 0..STARTING_BLOCKS {
            collection.cut_a_block(&mut chance, None);
        }
        visitors.nights_of_smoke = NIGHTS_ALREADY_KEPT;
    }
    // The console rather than the journal. The first traveller is summoned on
    // this same frame, so anything said here would be overwritten before a
    // player could read it — and the journal panel is only a few lines tall.
    info!(
        "{SWITCH}: {}",
        rehearsal_summary(&rehearsal, &collection, &progression)
    );
}

fn rehearsal_summary(
    rehearsal: &Rehearsal,
    collection: &Collection,
    progression: &Progression,
) -> String {
    let mut parts = vec![if rehearsal.repeat {
        "somebody arrives now, and again whenever the court empties".to_owned()
    } else {
        "somebody arrives now; F4 fetches the next one".to_owned()
    }];
    if let Some(story) = &rehearsal.story {
        parts.push(format!("Story pinned to {story}"));
    }
    if let Some(who) = &rehearsal.who {
        parts.push(format!("Arriving as {who}"));
    }
    parts.push(if rehearsal.cold {
        "Cold start: nothing to offer them".to_owned()
    } else {
        format!(
            "Fire lit, keys found, {} rations, {} blocks cut",
            progression.supply(SupplyId::Ration),
            collection.on_hand().len()
        )
    });
    parts.join(". ")
}

/// Sends the next party down the road: at once on the first frame, again after a
/// beat when `repeat` is set, and whenever F4 is pressed with an empty court.
pub fn summon_visitors(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    clock: Res<Clock>,
    rehearsal: Res<Rehearsal>,
    mut rehearsed: ResMut<Rehearsed>,
    mut visitors: ResMut<Visitors>,
) {
    if !rehearsal.on {
        return;
    }
    if visitors.party.is_some() {
        rehearsed.empty_for = 0.0;
        return;
    }
    rehearsed.empty_for += time.delta_secs();
    let first = rehearsed.summoned == 0;
    let due = rehearsal.repeat && rehearsed.empty_for >= BEAT_BETWEEN_VISITS;
    if !first && !due && !keys.just_pressed(KeyCode::F4) {
        return;
    }
    visitors.summon(*clock);
    rehearsed.summoned += 1;
    rehearsed.empty_for = 0.0;
}

/// Names what just walked down the road, so the story being looked at does not
/// have to be guessed from its first sentence. Empty in an ordinary game.
///
/// The switch is checked here rather than at the call site on purpose. A tag
/// like this is exactly the sort of thing that survives into a build somebody
/// ships, and a bare `if` around the call is one careless edit from leaking it;
/// asking the caller for the switch means there is nothing to remember.
///
/// Deliberately short, and deliberately at the front of the notice: the journal
/// panel is a fixed few lines tall and quietly drops whatever runs past the
/// bottom, so a tag appended to the end of a long arrival line is a tag nobody
/// ever sees.
pub fn announce(rehearsal: &Rehearsal, party: &crate::visitors::Party) -> String {
    if !rehearsal.on {
        return String::new();
    }
    format!("[{}/{}] ", party.profile().id, party.vignette)
}

/// The same arrival, said where nothing can crop it.
///
/// The journal panel is a fixed few lines and drops whatever runs past the
/// bottom, so the console is the channel that can be relied on: natively it is
/// the terminal `cargo run` is already printing to, and on the web it is the
/// browser console. Silent in an ordinary game, for the same reason `announce`
/// is: the switch is checked here so no call site has to remember to.
pub fn note_arrival(rehearsal: &Rehearsal, party: &crate::visitors::Party) {
    if !rehearsal.on {
        return;
    }
    info!(
        "{SWITCH}: {} · {} · {}",
        party.profile().id,
        party.vignette,
        party.address()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unset_switch_changes_nothing() {
        assert_eq!(Rehearsal::parse("").unwrap(), Rehearsal::default());
        assert!(!Rehearsal::parse("").unwrap().on);
        assert!(!Rehearsal::parse("off").unwrap().on);
        assert!(!Rehearsal::parse("0").unwrap().on);
    }

    /// `off` last wins, so a spec can be disabled by appending to it rather than
    /// by deleting the part worth keeping.
    #[test]
    fn off_clears_whatever_came_before_it() {
        assert_eq!(
            Rehearsal::parse("repeat,story=tace_dog,off").unwrap(),
            Rehearsal::default()
        );
    }

    #[test]
    fn every_token_turns_it_on() {
        for spec in ["now", "1", "on", "yes", "repeat", "cold", "story=tace_dog"] {
            assert!(Rehearsal::parse(spec).unwrap().on, "{spec}");
        }
    }

    #[test]
    fn tokens_combine_in_any_order_and_either_separator() {
        let expected = Rehearsal {
            on: true,
            repeat: true,
            story: Some("tace_dog".to_owned()),
            who: Some("old-hand".to_owned()),
            cold: false,
        };
        assert_eq!(
            Rehearsal::parse("repeat,story=tace_dog,who=old-hand").unwrap(),
            expected
        );
        assert_eq!(
            Rehearsal::parse("who=old-hand story=tace_dog repeat").unwrap(),
            expected
        );
    }

    /// A misspelt story id that quietly rehearsed a random one would cost more
    /// time than the switch saves, so it stops the game and says what is real.
    #[test]
    fn a_story_that_does_not_exist_is_refused_by_name() {
        let complaint = Rehearsal::parse("story=mara_greif").unwrap_err();
        assert!(complaint.contains("mara_greif"), "{complaint}");
        assert!(complaint.contains("mara_grief"), "{complaint}");

        let complaint = Rehearsal::parse("who=elder").unwrap_err();
        assert!(complaint.contains("old-hand"), "{complaint}");

        let complaint = Rehearsal::parse("hurry").unwrap_err();
        assert!(complaint.contains("story=<id>"), "{complaint}");
    }

    /// The words have to fit the body. Pinning the story about being talked to
    /// like a grown man has to bring the pair of children with it.
    #[test]
    fn a_pinned_story_brings_a_party_that_could_tell_it() {
        let wanted = Rehearsal::parse("story=amos_grown").unwrap().wanted();
        let profile = &PROFILES[wanted.profile.expect("a shape was chosen")];
        assert_eq!(profile.id, "siblings");
        assert!(profile.vignettes.contains(&"amos_grown"));

        for story in waystation_shared::vignettes() {
            let spec = format!("story={}", story.id);
            let wanted = Rehearsal::parse(&spec).unwrap().wanted();
            let index = wanted
                .profile
                .unwrap_or_else(|| panic!("{} has no shape that tells it", story.id));
            assert!(
                PROFILES[index].vignettes.contains(&story.id.as_str()),
                "{} was given to {}, which does not tell it",
                story.id,
                PROFILES[index].id
            );
        }
    }

    /// A named shape wins over the one the story would have chosen, because the
    /// point of naming it is to see the words in a body they were not written
    /// for.
    #[test]
    fn a_named_shape_overrides_the_one_the_story_suggests() {
        let wanted = Rehearsal::parse("story=amos_grown,who=walker")
            .unwrap()
            .wanted();
        assert_eq!(PROFILES[wanted.profile.expect("named")].id, "walker");
        assert_eq!(wanted.vignette.as_deref(), Some("amos_grown"));
    }

    /// The one thing in here a player could ever see. A debug tag left in front
    /// of the arrival line of a shipped build would be a small, permanent
    /// embarrassment, so the switch is checked inside `announce` rather than
    /// around it, and this is the check that keeps it that way.
    #[test]
    fn an_ordinary_game_is_told_nothing_about_rehearsal() {
        let mut visitors = Visitors::default();
        let party = visitors.arrive_wanted(
            &mut Chance::default(),
            crate::visitors::CURRENT_ERA,
            &Wanted::default(),
        );

        assert_eq!(announce(&Rehearsal::default(), party), "");
        for spec in ["", "off", "0"] {
            let quiet = Rehearsal::parse(spec).unwrap();
            assert_eq!(announce(&quiet, party), "", "{spec:?} leaked a tag");
        }

        let rehearsing = Rehearsal::parse("now").unwrap();
        assert!(announce(&rehearsing, party).starts_with('['));
    }

    #[test]
    fn an_ordinary_game_forces_nothing() {
        let wanted = Rehearsal::default().wanted();
        assert!(wanted.profile.is_none());
        assert!(wanted.vignette.is_none());
    }
}
