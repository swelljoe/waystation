//! The valley's clock.
//!
//! A day is ten minutes long. Nothing in the game announces what o'clock it is
//! beyond a line in the corner and the colour of the light, because the point of
//! the clock is not scheduling: it is to make the Scribe feel time passing, to
//! give sleeping a reason, and to give strangers somewhere to arrive *from*.
//!
//! Night is short and passes instantly in a bed, so a player who wants the days
//! to move can move them. A player who wants to stand in the dark may.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Ten minutes, wall clock, for one turn of the valley.
pub const DAY_SECONDS: f32 = 600.0;

/// The sky has finished brightening by here and starts dimming there. Between
/// them the light is flat: most of a day should be plain working daylight, or
/// the tint becomes something the player has to squint through rather than
/// something they notice.
const FIRST_FULL_LIGHT: f32 = 0.08;
const LIGHT_BEGINS_TO_FAIL: f32 = 0.72;
const LAST_LIGHT: f32 = 0.88;

/// Full dark still shows the world. This is a game about looking at a place, and
/// a screen the player cannot read is not atmosphere, it is a fault.
const DEEPEST_TINT: f32 = 0.58;

/// Sleeping is refused before this, so that a bed cannot be used to skip a day
/// the player simply does not feel like living through.
const EARLIEST_BEDTIME: f32 = LIGHT_BEGINS_TO_FAIL;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Dawn,
    Morning,
    Afternoon,
    Dusk,
    Night,
}

impl Phase {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Dawn => "Dawn",
            Self::Morning => "Morning",
            Self::Afternoon => "Afternoon",
            Self::Dusk => "Dusk",
            Self::Night => "Night",
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Clock {
    /// Days begin at one: the Scribe arrives on the first day, not the zeroth.
    pub day: u32,
    seconds: f32,
}

impl Default for Clock {
    fn default() -> Self {
        // Arrival is mid-morning. Starting at dawn would spend the player's
        // first minutes watching a tint fade instead of looking at the valley.
        Self {
            day: 1,
            seconds: DAY_SECONDS * 0.22,
        }
    }
}

impl Clock {
    /// How far through the day, in `0.0..1.0`.
    pub fn fraction(self) -> f32 {
        self.seconds / DAY_SECONDS
    }

    /// Advances the clock, returning true on the frame the date rolls over.
    pub fn tick(&mut self, seconds: f32) -> bool {
        self.seconds += seconds;
        if self.seconds < DAY_SECONDS {
            return false;
        }
        self.seconds %= DAY_SECONDS;
        self.day += 1;
        true
    }

    pub fn phase(self) -> Phase {
        let fraction = self.fraction();
        if fraction < FIRST_FULL_LIGHT {
            Phase::Dawn
        } else if fraction < 0.42 {
            Phase::Morning
        } else if fraction < LIGHT_BEGINS_TO_FAIL {
            Phase::Afternoon
        } else if fraction < LAST_LIGHT {
            Phase::Dusk
        } else {
            Phase::Night
        }
    }

    /// `1.0` in open daylight, falling to `0.0` at full dark.
    pub fn light(self) -> f32 {
        let fraction = self.fraction();
        if fraction < FIRST_FULL_LIGHT {
            fraction / FIRST_FULL_LIGHT
        } else if fraction < LIGHT_BEGINS_TO_FAIL {
            1.0
        } else if fraction < LAST_LIGHT {
            1.0 - (fraction - LIGHT_BEGINS_TO_FAIL) / (LAST_LIGHT - LIGHT_BEGINS_TO_FAIL)
        } else {
            0.0
        }
    }

    /// The wash laid over the whole screen. It leans amber while the light is
    /// still going and cold once it has gone, which is the difference between
    /// evening and night without needing to say either word.
    pub fn tint(self) -> Color {
        let light = self.light();
        let dark = 1.0 - light;
        let evening = Color::srgb(0.35, 0.13, 0.05);
        let deep_night = Color::srgb(0.03, 0.05, 0.16);
        let colour = evening.mix(&deep_night, dark.min(1.0));
        colour.with_alpha(dark * DEEPEST_TINT)
    }

    pub fn is_dark(self) -> bool {
        matches!(self.phase(), Phase::Night)
    }

    /// True once the light has begun to fail. A bed refuses before this.
    pub fn is_bedtime(self) -> bool {
        self.fraction() >= EARLIEST_BEDTIME
    }

    /// Skips whatever is left of tonight. Returns the morning that follows.
    pub fn sleep_until_morning(&mut self) -> u32 {
        // Sleeping at dusk still costs the whole night: waking at dusk on the
        // same day would let a player harvest one afternoon twice over.
        self.day += 1;
        self.seconds = DAY_SECONDS * FIRST_FULL_LIGHT;
        self.day
    }

    pub fn describe(self) -> String {
        format!("Day {} · {}", self.day, self.phase().label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_is_ten_minutes_and_rolls_over_exactly_once() {
        let mut clock = Clock {
            day: 4,
            seconds: DAY_SECONDS - 1.0,
        };
        assert!(!clock.tick(0.5), "half a second short is still the same day");
        assert!(clock.tick(1.0), "crossing midnight advances the date");
        assert_eq!(clock.day, 5);
        assert!(clock.fraction() < 0.01, "the new day starts at its start");
    }

    #[test]
    fn the_light_holds_flat_through_the_working_middle_of_the_day() {
        for fraction in [0.10, 0.30, 0.50, 0.70] {
            let clock = Clock {
                day: 1,
                seconds: DAY_SECONDS * fraction,
            };
            assert!(
                (clock.light() - 1.0).abs() < f32::EPSILON,
                "the day should not dim at {fraction}"
            );
            assert_eq!(clock.tint().alpha(), 0.0, "no wash over full daylight");
        }
    }

    #[test]
    fn the_light_falls_without_ever_coming_back_up_between_dusk_and_dark() {
        let mut previous = f32::MAX;
        let mut fraction = LIGHT_BEGINS_TO_FAIL;
        while fraction <= 1.0 {
            let clock = Clock {
                day: 1,
                seconds: DAY_SECONDS * fraction,
            };
            let light = clock.light();
            assert!(light <= previous, "the light brightened again at {fraction}");
            previous = light;
            fraction += 0.01;
        }
        assert_eq!(previous, 0.0, "night is fully dark");
    }

    #[test]
    fn full_dark_is_still_something_a_player_can_see_through() {
        let clock = Clock {
            day: 1,
            seconds: DAY_SECONDS * 0.95,
        };
        let alpha = clock.tint().alpha();
        assert!(
            (0.3..0.7).contains(&alpha),
            "night should read as night without blanking the screen, got {alpha}"
        );
    }

    #[test]
    fn a_bed_refuses_until_the_light_starts_to_go() {
        let noon = Clock {
            day: 2,
            seconds: DAY_SECONDS * 0.4,
        };
        assert!(!noon.is_bedtime());
        let dusk = Clock {
            day: 2,
            seconds: DAY_SECONDS * 0.8,
        };
        assert!(dusk.is_bedtime());
    }

    #[test]
    fn sleeping_at_dusk_costs_the_whole_night_rather_than_rewinding_the_day() {
        let mut clock = Clock {
            day: 2,
            seconds: DAY_SECONDS * 0.75,
        };
        assert_eq!(clock.sleep_until_morning(), 3);
        assert_eq!(clock.phase(), Phase::Morning);
        assert!(!clock.is_bedtime(), "you cannot sleep twice in a row");
    }
}
