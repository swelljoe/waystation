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
use waystation_npcgen::{ArtLicense, Cast, Casting, Era, Npc};
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

/// What colours the valley can still make.
///
/// Everything anyone wears was scavenged, or dyed with what grows in a dry
/// place. The story later brings real dye back and travellers start arriving in
/// colours that cost something; when that lands, this is the one line that has
/// to learn about it.
pub const CURRENT_ERA: Era = Era::Scavenged;

/// One body in an arriving party: who stands there, where they stand relative
/// to the party's spot, and the names they might give.
pub struct Body {
    /// The kind of person this is. The traveller themself is generated fresh on
    /// every arrival; this only fixes their age, because the authored shapes
    /// need it — "two of them, and one of those is small" has to be true.
    pub cast: Cast,
    /// A hand-made sheet to fall back on if generated art was never built. It
    /// is what the game shipped with before travellers were composited, and it
    /// is still what a build without an LPC checkout draws.
    pub art: &'static str,
    pub names: &'static [&'static str],
    pub offset: Vec2,
}

/// A kind of arrival: how many people, roughly what ages, and what they might
/// have come to say. Faces are not part of it — those are drawn fresh every
/// time, so the same shape arriving twice is not the same person twice.
pub struct Profile {
    pub id: &'static str,
    pub bodies: &'static [Body],
    /// Authored vignettes that suit this party. A lone walker cannot tell the
    /// story about carrying a child out of the camp below the Kiln, and a child
    /// cannot be the one who took a toll on this road twelve years ago. Every
    /// story lives in `content/vignettes.ron`; this is only who may tell it.
    pub vignettes: &'static [&'static str],
    /// What the Scribe sees before anything is said. A pool rather than one
    /// line: this is the sentence a player reads most often in a long game.
    pub sightings: &'static [&'static str],
}

pub const PROFILES: [Profile; 3] = [
    Profile {
        id: "walker",
        bodies: &[Body {
            cast: Cast::Grown,
            art: "people/walker.png",
            names: &[
                "Mara", "Sela", "Junia", "Rilla", "Hesper", "Ivy", "Noa", "Wren", "Tobin", "Amos",
                "Elias", "Ford", "Ruth", "Halden", "Perrin", "Ost", "Kell", "Sarn",
            ],
            offset: Vec2::ZERO,
        }],
        vignettes: &[
            "mara_grief",
            "oren_weariness",
            "kell_toll",
            "sarn_letter",
            "halden_water",
            "ivy_column",
            "noa_pace",
            "wren_seed",
            "ford_flood",
            "rilla_debt",
            "perrin_names",
        ],
        sightings: &[
            "One traveller, alone, with a much-mended pack.",
            "Somebody walking the ditch rather than the middle of the road.",
            "One person, keeping to the long way round the open ground.",
            "A walker alone, stopping every so often to look behind them.",
            "One set of footsteps. Whoever it is has been carrying that pack a while.",
        ],
    },
    Profile {
        id: "siblings",
        bodies: &[
            Body {
                cast: Cast::Youth,
                art: "people/elder-sibling.png",
                names: &[
                    "Tobin", "Amos", "Cass", "Rue", "Elias", "Ford", "Wren", "Sela",
                ],
                offset: Vec2::ZERO,
            },
            Body {
                cast: Cast::Child,
                art: "people/younger-sibling.png",
                names: &["Nell", "Pip", "Bry", "Tam", "Ada", "Sook", "Fen", "Mote"],
                offset: Vec2::new(30.0, -14.0),
            },
        ],
        vignettes: &[
            "fen_belonging",
            "cass_ration",
            "rue_milestone",
            "elias_quiet",
            "sela_offer",
            "amos_grown",
        ],
        sightings: &[
            "Two of them, and one of those is small.",
            "Two figures, close together, the taller one a half-step ahead.",
            "Two on the road. The small one is being kept on the far side.",
            "A pair, walking slowly enough that it is the smaller one setting the pace.",
        ],
    },
    Profile {
        id: "old-hand",
        bodies: &[Body {
            cast: Cast::Elder,
            art: "people/old-hand.png",
            names: &[
                "Oren", "Bertram", "Hale", "Sifter", "Corwin", "Marrow", "Auda", "Merrin", "Bel",
                "Tace",
            ],
            offset: Vec2::ZERO,
        }],
        vignettes: &[
            "oren_weariness",
            "mara_grief",
            "bertram_stations",
            "auda_knees",
            "marrow_words",
            "hale_toll",
            "corwin_terraces",
            "tace_dog",
        ],
        sightings: &[
            "Somebody old, looking at your roofline rather than at you.",
            "An old traveller, taking the slope one careful step at a time.",
            "Someone who has been walking a long time, and shows it.",
            "An old figure on the road, stopped, deciding.",
        ],
    },
];

/// A pinned arrival: this shape, or this story, or both.
///
/// Empty in an ordinary game — the dice decide. `crate::rehearsal` fills it in
/// so a writer can look at one story without waiting for it to come up.
#[derive(Clone, Debug, Default)]
pub struct Wanted {
    /// An index into `PROFILES`. Out-of-range is ignored rather than panicking;
    /// this only ever comes from a debug switch, and refusing to start the game
    /// over it would be worse than arriving as somebody else.
    pub profile: Option<usize>,
    pub vignette: Option<String>,
}

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
    /// One generated traveller per body, in the profile's order. What they are
    /// wearing, what their face is, whether they lean on a stick.
    pub people: Vec<Npc>,
    pub vignette: String,
    /// The first thing they say, drawn separately from the story that follows.
    pub opening: String,
    /// What the Scribe sees from across the court, before any of it.
    pub sighting: String,
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

    /// Everything they say, in the order they say it: the opening they chose
    /// on the way down the road, then the story they came to tell.
    #[must_use]
    pub fn spoken(&self) -> Vec<&str> {
        let mut lines = Vec::new();
        if !self.opening.is_empty() {
            lines.push(self.opening.as_str());
        }
        if let Some(vignette) = waystation_shared::vignette(&self.vignette) {
            lines.extend(vignette.lines.iter().map(String::as_str));
        }
        lines
    }

    /// One thing about how they look that is worth putting in the journal.
    ///
    /// Generated travellers vary in ways a fixed sighting line cannot describe,
    /// and a stick or a hood is the difference between "somebody is on the
    /// road" and a person. Only the first of a party is described; the sighting
    /// line already says how many there are.
    #[must_use]
    pub fn notable(&self) -> Option<&'static str> {
        let first = self.people.first()?;
        // Most telling first: a walking stick says more than a coat.
        if first.piece("weapon").is_some() {
            return Some("Whoever it is, they are leaning on a stick.");
        }
        if first.piece("hat").is_some() {
            return Some("The hood is up. You cannot see a face from here.");
        }
        if first.piece("backpack").is_some() {
            return Some("Everything they own looks to be on their back.");
        }
        if first.piece("neck").is_some() {
            return Some("Muffled to the eyes, though it is not cold enough for it.");
        }
        None
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

    /// Puts somebody on the road this instant, whatever the fire, the hour, or
    /// the dice would have said.
    ///
    /// Only `crate::rehearsal` calls it. It goes through the same schedule an
    /// ordinary arrival does rather than building a party directly, so a
    /// rehearsed visit is the same visit a player gets and cannot drift from it.
    pub fn summon(&mut self, clock: Clock) {
        // Claiming the day as rolled keeps the ordinary roll from clearing this
        // on the next frame, which is exactly what it is there to do.
        self.rolled_for_day = clock.day;
        self.scheduled = Some((clock.day, clock.fraction()));
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

    /// Builds an arriving party.
    ///
    /// Everything about them is drawn fresh: the shape of the arrival, the
    /// people standing in it, their names, the first thing they say, and which
    /// story they tell. Two lone walkers a month apart share nothing but the
    /// fact that they came alone.
    /// The game itself always goes through `arrive_wanted`, because the debug
    /// switch has to be able to pin a story; this is the same call with nothing
    /// pinned, kept so the tests read as what an ordinary day does.
    #[cfg(test)]
    pub fn arrive(&mut self, chance: &mut Chance, era: Era) -> &Party {
        self.arrive_wanted(chance, era, &Wanted::default())
    }

    /// The same arrival, with a shape or a story pinned.
    ///
    /// Nothing in the game asks for this; `crate::rehearsal` does, so a writer
    /// can look at one particular story without waiting for the dice to offer
    /// it. Everything not pinned is still drawn fresh, which is why a pinned
    /// story still arrives on a different face every time.
    pub fn arrive_wanted(&mut self, chance: &mut Chance, era: Era, wanted: &Wanted) -> &Party {
        let profile_index = wanted
            .profile
            .filter(|index| *index < PROFILES.len())
            .unwrap_or_else(|| chance.below(PROFILES.len()));
        let profile = &PROFILES[profile_index];
        let names = profile
            .bodies
            .iter()
            .map(|body| (*chance.pick(body.names).unwrap_or(&"the stranger")).to_owned())
            .collect();
        let people = profile
            .bodies
            .iter()
            .map(|body| {
                waystation_npcgen::generate_with(
                    chance.seed(),
                    Casting {
                        era,
                        // In-game art is only ever a texture standing in the
                        // court, never a flat image beside purchased tilesets,
                        // so share-alike pieces are welcome here. Screenshots
                        // are the place that bar goes up.
                        license: ArtLicense::ShareAlike,
                        cast: body.cast,
                    },
                )
            })
            .collect();
        let vignette = wanted.vignette.clone().unwrap_or_else(|| {
            (*chance.pick(profile.vignettes).unwrap_or(&"mara_grief")).to_owned()
        });
        let sighting = (*chance
            .pick(profile.sightings)
            .unwrap_or(&"Somebody is on the road."))
        .to_owned();
        let openings = waystation_shared::openings_for(profile.bodies.len());
        let opening = chance
            .pick(&openings)
            .map_or_else(String::new, |opening| opening.line.clone());
        self.visits_received += 1;
        self.party.insert(Party {
            profile: profile_index,
            names,
            people,
            vignette,
            opening,
            sighting,
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
        visitors.arrive(&mut chance, CURRENT_ERA);
        visitors.party.as_mut().expect("party").stage = Stage::Waiting;
        assert!(!visitors.tick_patience(PATIENCE_SECONDS - 1.0));
        assert!(visitors.tick_patience(2.0), "patience should have run out");
        assert_eq!(visitors.party.expect("party").stage, Stage::Leaving);
    }

    #[test]
    fn nobody_gets_impatient_in_the_middle_of_their_own_sentence() {
        let mut visitors = Visitors::default();
        let mut chance = Chance::default();
        visitors.arrive(&mut chance, CURRENT_ERA);
        visitors.party.as_mut().expect("party").stage = Stage::Telling;
        assert!(!visitors.tick_patience(PATIENCE_SECONDS * 10.0));
        assert_eq!(visitors.party.expect("party").stage, Stage::Telling);
    }

    #[test]
    fn a_guest_given_a_room_stays_the_night_and_a_refused_one_leaves() {
        let mut visitors = Visitors::default();
        let mut chance = Chance::default();
        visitors.arrive(&mut chance, CURRENT_ERA);
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

    /// The story is the longest thing a traveller says, so hearing it twice is
    /// what makes a stranger stop being a stranger. Every shape of arrival needs
    /// a pool deep enough that a player finishing the game has not heard them
    /// all — and no shape may quietly fall back to a single story.
    #[test]
    fn every_profile_has_enough_to_say() {
        for profile in &PROFILES {
            assert!(
                profile.vignettes.len() >= 5,
                "{} has only {} stories to tell",
                profile.id,
                profile.vignettes.len()
            );
        }
    }

    /// A story nobody can tell is a story nobody reads. Authoring one and
    /// forgetting to hand it to a profile is the easy mistake here, and it is
    /// silent: the game keeps working and the words never appear.
    #[test]
    fn every_authored_story_has_somebody_who_could_tell_it() {
        let told: std::collections::HashSet<&str> = PROFILES
            .iter()
            .flat_map(|profile| profile.vignettes.iter().copied())
            .collect();
        for authored in waystation_shared::vignettes() {
            assert!(
                told.contains(authored.id.as_str()),
                "{} is authored but no profile in PROFILES tells it",
                authored.id
            );
        }
    }

    /// Every arrival gets one, and the pool is what stops a player reading the
    /// same sentence forty times.
    #[test]
    fn every_profile_has_more_than_one_thing_to_look_like() {
        for profile in &PROFILES {
            assert!(
                profile.sightings.len() >= 4,
                "{} has {} sightings",
                profile.id,
                profile.sightings.len()
            );
            for sighting in profile.sightings {
                assert!(
                    !sighting.trim().is_empty(),
                    "{} has a blank one",
                    profile.id
                );
            }
            assert!(
                !waystation_shared::openings_for(profile.bodies.len()).is_empty(),
                "nothing in content/openings.ron suits a party of {} — {} would \
                 arrive with nothing to say",
                profile.bodies.len(),
                profile.id
            );
        }
    }

    /// The whole point: two arrivals of the same shape are two different sets
    /// of people, not one set of people twice.
    #[test]
    fn two_arrivals_of_the_same_shape_are_different_people() {
        let mut chance = Chance::default();
        let mut seen = std::collections::HashSet::new();
        let mut shapes = std::collections::HashSet::new();
        for _ in 0..60 {
            let mut visitors = Visitors::default();
            let party = visitors.arrive(&mut chance, CURRENT_ERA).clone();
            assert_eq!(
                party.people.len(),
                party.profile().bodies.len(),
                "somebody in the party was never generated"
            );
            for (npc, body) in party.people.iter().zip(party.profile().bodies) {
                assert_eq!(npc.cast, body.cast, "the wrong sort of person arrived");
                seen.insert(npc.describe());
            }
            shapes.insert(party.profile);
        }
        assert!(seen.len() > 60, "only {} distinct travellers", seen.len());
        assert!(shapes.len() > 1, "only one kind of arrival ever happened");
    }

    /// Distinct faces telling the same three stories would be worse than the
    /// hand-drawn four, not better — the repetition would land on words rather
    /// than on pixels. A season of arrivals has to be a season of stories.
    #[test]
    fn a_season_of_arrivals_is_not_the_same_story_over_and_over() {
        const ARRIVALS: usize = 120;
        let mut chance = Chance::default();
        let mut told = std::collections::HashMap::new();
        for _ in 0..ARRIVALS {
            let mut visitors = Visitors::default();
            let party = visitors.arrive(&mut chance, CURRENT_ERA);
            *told.entry(party.vignette.clone()).or_insert(0_usize) += 1;
        }
        assert!(
            told.len() >= 15,
            "only {} distinct stories in {ARRIVALS} arrivals",
            told.len()
        );
        let commonest = told.values().max().copied().unwrap_or_default();
        assert!(
            commonest * 5 <= ARRIVALS,
            "one story turned up {commonest} times in {ARRIVALS} arrivals"
        );
    }

    /// A summons is the whole of the debug switch's power over the calendar: no
    /// fire, no dice, no waiting for the hour. It has to survive the ordinary
    /// roll that runs on the very next frame and would otherwise clear it.
    #[test]
    fn a_summons_beats_the_fire_the_dice_and_the_hour() {
        let mut visitors = Visitors::default();
        let mut chance = Chance::default();
        let clock = Clock::default();
        assert!(!visitors.arrival_is_due(clock), "nobody was due");

        visitors.summon(clock);
        visitors.roll_for_today(clock, false, &mut chance);
        assert!(
            visitors.arrival_is_due(clock),
            "the ordinary roll cleared a summons"
        );
        assert!(!visitors.arrival_is_due(clock), "one summons, one arrival");
    }

    /// Pinning is what lets a writer look at one story without waiting for it to
    /// come up. What is not pinned still has to be drawn fresh, or rehearsing
    /// would show the same person every time and hide exactly the variation the
    /// generator exists for.
    #[test]
    fn a_pinned_arrival_keeps_its_pin_and_varies_everything_else() {
        let mut chance = Chance::default();
        let wanted = Wanted {
            profile: Some(1),
            vignette: Some("sela_offer".to_owned()),
        };
        let mut faces = std::collections::HashSet::new();
        for _ in 0..20 {
            let mut visitors = Visitors::default();
            let party = visitors.arrive_wanted(&mut chance, CURRENT_ERA, &wanted);
            assert_eq!(party.vignette, "sela_offer");
            assert_eq!(party.profile().id, "siblings");
            faces.insert(party.people[0].describe());
        }
        assert!(faces.len() > 15, "only {} distinct faces", faces.len());
    }

    /// A profile index out of range comes only from a debug switch, and refusing
    /// to start the game over it would be worse than arriving as somebody else.
    #[test]
    fn a_nonsense_pin_still_produces_a_party() {
        let mut visitors = Visitors::default();
        let mut chance = Chance::default();
        let wanted = Wanted {
            profile: Some(99),
            vignette: None,
        };
        let party = visitors.arrive_wanted(&mut chance, CURRENT_ERA, &wanted);
        assert!(waystation_shared::vignette(&party.vignette).is_some());
    }

    /// The opening is what they say first, before any story of their own.
    #[test]
    fn a_party_speaks_its_opening_before_its_story() {
        let mut visitors = Visitors::default();
        let mut chance = Chance::default();
        let party = visitors.arrive(&mut chance, CURRENT_ERA).clone();

        let spoken = party.spoken();
        assert_eq!(spoken.first().copied(), Some(party.opening.as_str()));
        let story = waystation_shared::vignette(&party.vignette).expect("an authored story");
        assert_eq!(spoken.len(), 1 + story.lines.len());
        assert_eq!(spoken[1], story.lines[0]);
    }

    /// A party with no opening — which nothing produces today, but a content
    /// edit could — still has a story to tell rather than an empty screen.
    #[test]
    fn a_party_with_nothing_to_open_with_still_tells_its_story() {
        let mut visitors = Visitors::default();
        let mut chance = Chance::default();
        let mut party = visitors.arrive(&mut chance, CURRENT_ERA).clone();
        party.opening = String::new();

        let spoken = party.spoken();
        assert!(!spoken.is_empty());
        assert!(!spoken.iter().any(|line| line.is_empty()));
    }

    #[test]
    fn a_pair_is_addressed_as_two_people() {
        let mut visitors = Visitors::default();
        let mut chance = Chance::default();
        let party = visitors.arrive(&mut chance, CURRENT_ERA).clone();
        let mut pair = party;
        pair.names = vec!["Tobin".to_owned(), "Nell".to_owned()];
        assert_eq!(pair.address(), "Tobin and Nell");
        pair.names = vec!["Mara".to_owned()];
        assert_eq!(pair.address(), "Mara");
    }
}
