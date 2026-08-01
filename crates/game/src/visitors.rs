//! Who comes down the road, and when.
//!
//! Nobody arrives because the story says so. A fire in a dead valley is the only
//! advertisement the waystation has, and it is a frightening one: smoke means
//! strangers, and strangers are what everyone left alive has learned to avoid.
//! So the first nights of a lit hearth pass with nobody at all. Only after the
//! fire has kept burning — after it has stopped looking like a raiding party and
//! started looking like a household — does anyone risk the walk down.
//!
//! Once they do come, they do not wait long. A stranger standing in the open
//! beside a building they do not know is a stranger taking a risk, and if the
//! keeper of the fire does not come out to meet them, the sensible thing is to
//! keep walking.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use waystation_shared::InterpretResponse;

use crate::chance::Chance;
use crate::daylight::Clock;

/// Nights of steady fire before anyone is willing to be seen approaching. The
/// second evening is far too early; that was the old scripted arc's mistake.
const NIGHTS_OF_SMOKE_BEFORE_ANYONE_RISKS_IT: u32 = 3;

/// How likely an arrival is on a given day, once the fire has been burning long
/// enough to be believed. It climbs as the fire keeps up and then levels off:
/// a waystation becomes known, but the wastes never become busy.
const FIRST_ODDS: f32 = 0.30;
const ODDS_GAINED_PER_NIGHT: f32 = 0.06;
const BEST_ODDS: f32 = 0.65;

/// Arrivals land in the working middle of the day. Nobody walks up to a strange
/// building in the dark, and nobody sets out before it is light enough to see.
const EARLIEST_ARRIVAL: f32 = 0.18;
const LATEST_ARRIVAL: f32 = 0.62;

/// How long a stranger will stand in the open before deciding this was a bad
/// idea. Roughly an eighth of a day — long enough to cross the lot and speak,
/// short enough that ignoring someone costs you the meeting.
pub const PATIENCE_SECONDS: f32 = 70.0;

/// A guest who has been given a room is not going anywhere; the long patience is
/// only there so a stuck party cannot loiter for the rest of the game.
const GUEST_PATIENCE_SECONDS: f32 = 600.0;

/// One body in an arriving party: its sheet, where it stands relative to the
/// party's spot, and the names it might give.
pub struct Body {
    pub art: &'static str,
    pub names: &'static [&'static str],
    pub offset: Vec2,
}

/// A kind of person who might come down the road. Art and names are separate so
/// the same sheet can arrive twice across a long game without being the same
/// person, which is the cheapest replayability there is.
pub struct Profile {
    #[cfg_attr(not(test), allow(dead_code))]
    pub id: &'static str,
    pub bodies: &'static [Body],
    /// Authored vignettes that suit this party. A lone walker cannot tell the
    /// story about being turned out of a camp with her brother in tow.
    pub vignettes: &'static [&'static str],
    /// What the Scribe sees before anything is said.
    pub sighting: &'static str,
}

pub const PROFILES: [Profile; 3] = [
    Profile {
        id: "walker",
        bodies: &[Body {
            art: "people/walker.png",
            names: &[
                "Mara", "Sela", "Junia", "Rilla", "Hesper", "Ivy", "Noa", "Wren",
            ],
            offset: Vec2::ZERO,
        }],
        vignettes: &["mara_grief", "oren_weariness"],
        sighting: "A woman, alone, with a much-mended pack.",
    },
    Profile {
        id: "siblings",
        bodies: &[
            Body {
                art: "people/elder-sibling.png",
                names: &["Tobin", "Amos", "Cass", "Rue", "Elias", "Ford"],
                offset: Vec2::ZERO,
            },
            Body {
                art: "people/younger-sibling.png",
                names: &["Nell", "Pip", "Bry", "Tam", "Ada", "Sook"],
                offset: Vec2::new(30.0, -14.0),
            },
        ],
        vignettes: &["fen_belonging"],
        sighting: "Two of them, and one of those is small.",
    },
    Profile {
        id: "old-hand",
        bodies: &[Body {
            art: "people/old-hand.png",
            names: &["Oren", "Bertram", "Hale", "Sifter", "Corwin", "Marrow"],
            offset: Vec2::ZERO,
        }],
        vignettes: &["oren_weariness", "mara_grief"],
        sighting: "An old man, looking at your roofline rather than at you.",
    },
];

/// Where a party is in its visit. Nothing here forces the player's hand: every
/// state except the walking ones is left by the player choosing to leave it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    /// Walking in from the road, not yet close enough to speak to.
    Approaching,
    /// Standing in the open. Patience is running.
    Waiting,
    /// Telling their story, one line at a time.
    Telling,
    /// The Scribe is listening for the need beneath the words.
    Listening,
    /// The player is choosing what, if anything, to offer.
    Deciding,
    /// Leafing through the cards on hand for one to hand over.
    Choosing,
    /// Given a room, and gone into it for the night.
    Lodging,
    /// Walking back out to the road.
    Leaving,
}

impl Stage {
    /// True while the party owns the screen. Movement and world interaction stop
    /// so the player is not walking away mid-sentence.
    pub const fn holds_the_screen(self) -> bool {
        matches!(
            self,
            Self::Telling | Self::Listening | Self::Deciding | Self::Choosing
        )
    }

    pub const fn is_walking(self) -> bool {
        matches!(self, Self::Approaching | Self::Leaving)
    }

    /// Whether the bodies should be drawn at all.
    pub const fn is_present(self) -> bool {
        !matches!(self, Self::Lodging)
    }
}

/// What the party was given. Kept so the farewell can say the true thing rather
/// than a generic one, and so a second helping cannot be pressed on them.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Given {
    pub food: bool,
    pub room: Option<String>,
    pub card: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Party {
    pub profile: usize,
    /// One name per body, in the profile's order.
    pub names: Vec<String>,
    pub vignette: String,
    pub stage: Stage,
    pub line: usize,
    pub patience: f32,
    pub need: Option<InterpretResponse>,
    pub given: Given,
    /// Told their story already, so a second conversation goes straight to the
    /// choosing rather than making them recite it again.
    pub has_spoken: bool,
    /// Set when the party has finished walking out and can be despawned.
    pub gone: bool,
}

impl Party {
    /// How to address them. Two names read as two people, which is the whole
    /// point of the pair arriving together.
    pub fn address(&self) -> String {
        match self.names.as_slice() {
            [] => "the stranger".to_owned(),
            [one] => one.clone(),
            [first, rest @ ..] => format!("{first} and {}", rest.join(" and ")),
        }
    }

    pub const fn profile(&self) -> &'static Profile {
        &PROFILES[self.profile % PROFILES.len()]
    }

    /// True once they have gone as far into the visit as they can without the
    /// player offering anything more.
    pub const fn can_be_greeted(&self) -> bool {
        matches!(self.stage, Stage::Waiting)
    }
}

/// The valley's whole social calendar.
#[derive(Resource, Default)]
pub struct Visitors {
    pub party: Option<Party>,
    /// Nights the hearth has been alight. Arrivals depend on this, not on days.
    pub nights_of_smoke: u32,
    /// Set when an arrival has been rolled for today: `(day, fraction)`.
    scheduled: Option<(u32, f32)>,
    /// The last day an arrival was rolled for, so the dice are thrown once.
    rolled_for_day: u32,
    pub visits_received: u32,
}

impl Visitors {
    /// The chance of anyone coming today. Zero until the fire has been burning
    /// long enough for the smoke to look like a household rather than a threat.
    pub fn odds_today(&self) -> f32 {
        if self.nights_of_smoke < NIGHTS_OF_SMOKE_BEFORE_ANYONE_RISKS_IT {
            return 0.0;
        }
        let earned = self.nights_of_smoke - NIGHTS_OF_SMOKE_BEFORE_ANYONE_RISKS_IT;
        #[allow(clippy::cast_precision_loss)]
        let climb = earned as f32 * ODDS_GAINED_PER_NIGHT;
        (FIRST_ODDS + climb).min(BEST_ODDS)
    }

    /// Rolls once per day for whether somebody comes, and at what hour. Called
    /// every frame; it is a no-op after the first call on a given day.
    pub fn roll_for_today(&mut self, clock: Clock, fire_is_lit: bool, chance: &mut Chance) {
        if self.rolled_for_day == clock.day {
            return;
        }
        self.rolled_for_day = clock.day;
        self.scheduled = None;
        if !fire_is_lit || self.party.is_some() {
            return;
        }
        if chance.odds(self.odds_today()) {
            let hour = chance.between(EARLIEST_ARRIVAL, LATEST_ARRIVAL);
            self.scheduled = Some((clock.day, hour));
        }
    }

    /// True on the frame the scheduled hour arrives. Consumes the schedule.
    pub fn arrival_is_due(&mut self, clock: Clock) -> bool {
        let Some((day, hour)) = self.scheduled else {
            return false;
        };
        if clock.day != day || clock.fraction() < hour {
            return false;
        }
        self.scheduled = None;
        true
    }

    /// Builds an arriving party. The profile, the names, and which story they
    /// tell are all drawn fresh, so meeting the same sheet twice is not meeting
    /// the same person twice.
    pub fn arrive(&mut self, chance: &mut Chance) -> &Party {
        let profile_index = chance.below(PROFILES.len());
        let profile = &PROFILES[profile_index];
        let names = profile
            .bodies
            .iter()
            .map(|body| (*chance.pick(body.names).unwrap_or(&"the stranger")).to_owned())
            .collect();
        let vignette = (*chance.pick(profile.vignettes).unwrap_or(&"mara_grief")).to_owned();
        self.visits_received += 1;
        self.party.insert(Party {
            profile: profile_index,
            names,
            vignette,
            stage: Stage::Approaching,
            line: 0,
            patience: PATIENCE_SECONDS,
            need: None,
            given: Given::default(),
            has_spoken: false,
            gone: false,
        })
    }

    /// Counts down the patience of a party standing in the open. Returns true on
    /// the frame they give up.
    pub fn tick_patience(&mut self, seconds: f32) -> bool {
        let Some(party) = self.party.as_mut() else {
            return false;
        };
        if party.stage != Stage::Waiting {
            return false;
        }
        party.patience -= seconds;
        if party.patience > 0.0 {
            return false;
        }
        party.stage = Stage::Leaving;
        true
    }

    /// Morning: anyone who took a room comes back out to say goodbye. They are
    /// patient about it — they have already decided you are not a danger.
    pub fn wake_guests(&mut self) {
        if let Some(party) = self.party.as_mut() {
            if party.stage == Stage::Lodging {
                party.stage = Stage::Waiting;
                party.patience = GUEST_PATIENCE_SECONDS;
            }
        }
    }

    /// Where the visit goes when the player closes the choosing screen: into a
    /// room if one was offered, back onto the road otherwise.
    pub fn finish_deciding(&mut self) {
        if let Some(party) = self.party.as_mut() {
            party.stage = if party.given.room.is_some() && party.stage == Stage::Deciding {
                Stage::Lodging
            } else {
                Stage::Leaving
            };
        }
    }

    pub fn clear_departed(&mut self) {
        if self.party.as_ref().is_some_and(|party| party.gone) {
            self.party = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nobody_comes_to_a_cold_hearth_however_many_days_pass() {
        let mut visitors = Visitors::default();
        let mut chance = Chance::default();
        for day in 1..40 {
            visitors.roll_for_today(Clock::at(day, 0.1), false, &mut chance);
            assert!(
                !visitors.arrival_is_due(Clock::at(day, 0.9)),
                "someone walked up to a dead building on day {day}"
            );
        }
    }

    #[test]
    fn the_first_nights_of_smoke_still_bring_nobody() {
        let mut visitors = Visitors::default();
        for nights in 0..NIGHTS_OF_SMOKE_BEFORE_ANYONE_RISKS_IT {
            visitors.nights_of_smoke = nights;
            assert!(
                visitors.odds_today() < f32::EPSILON,
                "a fire {nights} nights old is still a warning, not an invitation"
            );
        }
        visitors.nights_of_smoke = NIGHTS_OF_SMOKE_BEFORE_ANYONE_RISKS_IT;
        assert!(visitors.odds_today() > 0.0);
    }

    #[test]
    fn a_long_kept_fire_becomes_known_but_never_busy() {
        let visitors = Visitors {
            nights_of_smoke: 400,
            ..Visitors::default()
        };
        assert!(
            (visitors.odds_today() - BEST_ODDS).abs() < f32::EPSILON,
            "the wastes should never fill up with travellers"
        );
    }

    #[test]
    fn the_dice_are_thrown_once_a_day_and_the_hour_falls_in_daylight() {
        let mut visitors = Visitors {
            nights_of_smoke: 40,
            ..Visitors::default()
        };
        let mut chance = Chance::default();
        let mut arrivals = 0;
        for day in 1..300 {
            visitors.roll_for_today(Clock::at(day, 0.05), true, &mut chance);
            // Rolling again the same day must not get a second chance at it.
            let before = visitors.scheduled;
            visitors.roll_for_today(Clock::at(day, 0.06), true, &mut chance);
            assert_eq!(before, visitors.scheduled, "day {day} was rolled twice");
            if let Some((_, hour)) = visitors.scheduled {
                assert!((EARLIEST_ARRIVAL..=LATEST_ARRIVAL).contains(&hour));
                arrivals += 1;
            }
        }
        assert!(
            (100..250).contains(&arrivals),
            "{arrivals} arrivals in 299 well-kept days is not roughly two in three"
        );
    }

    #[test]
    fn an_arrival_waits_for_its_hour_and_then_fires_exactly_once() {
        let mut visitors = Visitors {
            scheduled: Some((6, 0.4)),
            ..Visitors::default()
        };
        assert!(!visitors.arrival_is_due(Clock::at(6, 0.2)), "too early");
        assert!(!visitors.arrival_is_due(Clock::at(5, 0.9)), "wrong day");
        assert!(visitors.arrival_is_due(Clock::at(6, 0.41)));
        assert!(
            !visitors.arrival_is_due(Clock::at(6, 0.5)),
            "the same arrival landed twice"
        );
    }

    #[test]
    fn a_stranger_left_standing_gives_up_and_walks_on() {
        let mut visitors = Visitors::default();
        let mut chance = Chance::default();
        visitors.arrive(&mut chance);
        visitors.party.as_mut().expect("party").stage = Stage::Waiting;
        assert!(!visitors.tick_patience(PATIENCE_SECONDS - 1.0));
        assert!(visitors.tick_patience(2.0), "patience should have run out");
        assert_eq!(visitors.party.expect("party").stage, Stage::Leaving);
    }

    #[test]
    fn nobody_gets_impatient_in_the_middle_of_their_own_sentence() {
        let mut visitors = Visitors::default();
        let mut chance = Chance::default();
        visitors.arrive(&mut chance);
        visitors.party.as_mut().expect("party").stage = Stage::Telling;
        assert!(!visitors.tick_patience(PATIENCE_SECONDS * 10.0));
        assert_eq!(visitors.party.expect("party").stage, Stage::Telling);
    }

    #[test]
    fn a_guest_given_a_room_stays_the_night_and_a_refused_one_leaves() {
        let mut visitors = Visitors::default();
        let mut chance = Chance::default();
        visitors.arrive(&mut chance);
        let party = visitors.party.as_mut().expect("party");
        party.stage = Stage::Deciding;
        party.given.room = Some("motel-room-01".to_owned());
        visitors.finish_deciding();
        assert_eq!(
            visitors.party.as_ref().expect("party").stage,
            Stage::Lodging
        );

        visitors.wake_guests();
        let party = visitors.party.as_ref().expect("party");
        assert_eq!(party.stage, Stage::Waiting);
        assert!(
            party.patience > PATIENCE_SECONDS,
            "a guest saying goodbye should not bolt"
        );

        let party = visitors.party.as_mut().expect("party");
        party.stage = Stage::Deciding;
        party.given.room = None;
        visitors.finish_deciding();
        assert_eq!(visitors.party.expect("party").stage, Stage::Leaving);
    }

    #[test]
    fn every_profile_names_its_bodies_and_points_at_real_vignettes() {
        for profile in &PROFILES {
            assert!(!profile.bodies.is_empty(), "{} has nobody", profile.id);
            for body in profile.bodies {
                assert!(!body.names.is_empty(), "{} has a nameless body", profile.id);
            }
            for id in profile.vignettes {
                assert!(
                    waystation_shared::vignette(id).is_some(),
                    "{} tells {id}, which is not authored",
                    profile.id
                );
            }
        }
    }

    #[test]
    fn a_pair_is_addressed_as_two_people() {
        let mut visitors = Visitors::default();
        let mut chance = Chance::default();
        let party = visitors.arrive(&mut chance).clone();
        let mut pair = party;
        pair.names = vec!["Tobin".to_owned(), "Nell".to_owned()];
        assert_eq!(pair.address(), "Tobin and Nell");
        pair.names = vec!["Mara".to_owned()];
        assert_eq!(pair.address(), "Mara");
    }
}
