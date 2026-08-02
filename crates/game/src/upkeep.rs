//! What a day at the waystation costs.
//!
//! Nothing else in the game charges rent. Repairs spend supplies once and the
//! result stands for ever; the hearth was lit on day three and would still be
//! burning on day three hundred. That makes hospitality free, and free
//! hospitality is not hospitality — a bowl given away has to be a bowl the
//! Scribe was going to eat.
//!
//! So the night is settled here. The fire burns wood it has to have been fed.
//! The Scribe eats, out of the pot if there is anything in it. Sleeping under a
//! sound roof is what makes the night dry. Fed, warm, and dry is a good day, and
//! the count of good days is the only score this game keeps.
//!
//! None of it can be finished. A mended roof stays mended; a full pot does not
//! stay full.

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::progression::{Progression, SupplyId};

/// Strength is carried in eighths of a bowl, so that a pot can be let down with
/// water several times over before it stops being food.
const FULL: u16 = 8;

/// What one ration is worth in the pot. Two bowls at full strength, which is
/// the whole argument for a stew: the same food, stretched.
const BOWLS_PER_RATION: u16 = 2;

/// Below this a bowl is hot water. It warms the hands and feeds nobody.
const NOURISHING: u16 = 2;

/// The pot is a pot. It will not hold a winter.
const POT_CAPACITY: u16 = 12;

/// Nights of wood the hearth will hold banked at one time. Beyond this the pile
/// is just a pile, and the Scribe would rather it stayed dry outside.
const MAX_BANKED_NIGHTS: u8 = 4;

/// A fallen log, split, is two nights of fire — three in a room whose walls
/// have all been mended, because most of a fire goes out through the gaps.
const NIGHTS_PER_LOG: u8 = 2;

/// Kindling burns hot and fast: a whole armful is one night.
const KINDLING_PER_FEED: u16 = 3;

/// What went into the fire, for the line that reports it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fuel {
    Log,
    Kindling,
}

/// What the Scribe had for supper. The difference is entirely in the telling,
/// except that half a bowl does not settle a hungry stretch.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Meal {
    /// A bowl out of the pot with something in it.
    Bowl,
    /// The half kept back after splitting the last of it with a stranger.
    Half,
    /// Grain and roots eaten as they came, because the pot was empty.
    Raw,
    /// What is left when a pot has been stretched past the point of food.
    HotWater,
    #[default]
    Nothing,
}

/// The perpetual stew. Rations go in, water goes in, bowls come out, and it is
/// never emptied and never washed.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Stew {
    bowls: u16,
    /// Everything actually in the pot, in eighths of a bowl.
    substance: u16,
}

impl Stew {
    pub const fn bowls(&self) -> u16 {
        self.bowls
    }

    /// Eighths of a bowl per bowl: `FULL` for a pot that has only had food in
    /// it, falling every time it is let down with water.
    pub const fn strength(&self) -> u16 {
        match self.substance.checked_div(self.bowls) {
            Some(strength) => strength,
            None => 0,
        }
    }

    /// True while a bowl out of this pot is still supper rather than a hot drink.
    pub const fn is_food(&self) -> bool {
        self.bowls > 0 && self.strength() >= NOURISHING
    }

    pub const fn has_room_for_a_ration(&self) -> bool {
        self.bowls + BOWLS_PER_RATION <= POT_CAPACITY
    }

    /// Water only helps while there is something to spread. Adding it to a pot
    /// that is already hot water is not a decision worth offering.
    pub const fn worth_stretching(&self) -> bool {
        self.bowls < POT_CAPACITY && self.strength() > NOURISHING
    }

    const fn add_a_ration(&mut self) {
        self.bowls += BOWLS_PER_RATION;
        self.substance += BOWLS_PER_RATION * FULL;
    }

    /// A canful and an hour. One more bowl and everything in the pot is thinner
    /// for it, which is the trade.
    const fn stretch(&mut self) {
        self.bowls += 1;
    }

    /// Takes a bowl out, at whatever the pot is worth. The rounding goes
    /// against the pot, so one left standing long enough thins out on its own.
    const fn take_a_bowl(&mut self) -> u16 {
        if self.bowls == 0 {
            return 0;
        }
        let strength = self.substance / self.bowls;
        self.substance -= strength;
        self.bowls -= 1;
        strength
    }

    /// What a bowl of this is like, said properly. For the journal, where there
    /// is room for a sentence.
    pub const fn quality(&self) -> &'static str {
        match self.strength() {
            0..=1 => "clear water with a memory of grain in it",
            2..=3 => "thin",
            4..=5 => "honest",
            _ => "thick enough to stand a spoon in",
        }
    }

    /// The same judgement in one word, for the ledger, which is a narrow column
    /// and not a place for a sentence.
    pub const fn shorthand(&self) -> &'static str {
        match self.strength() {
            0..=1 => "watery",
            2..=3 => "thin",
            4..=5 => "honest",
            _ => "thick",
        }
    }

    /// For the ledger.
    pub fn describe(&self) -> String {
        self.say(Self::shorthand)
    }

    /// For the journal, where the Scribe is talking rather than counting.
    pub fn describe_at_length(&self) -> String {
        self.say(Self::quality)
    }

    fn say(&self, judgement: impl Fn(&Self) -> &'static str) -> String {
        match self.bowls {
            0 => "the pot is empty".to_owned(),
            1 => format!("one bowl left, {}", judgement(self)),
            count => format!("{count} bowls, {}", judgement(self)),
        }
    }
}

/// The account of one night, settled at the turn of the day.
// Four bools, and they are the four things a day is judged on. Folding them into
// an enum would mean naming sixteen kinds of night nobody needs a name for.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Night {
    pub fed: bool,
    pub warm: bool,
    pub dry: bool,
    pub meal: Meal,
    /// True on the one morning the hearth is found cold, so the game can say so
    /// once instead of every day afterwards.
    pub fire_went_out: bool,
}

impl Night {
    pub const fn was_good(self) -> bool {
        self.fed && self.warm && self.dry
    }

    /// The one line the morning gets. It reports the night and never what to do
    /// about it, like everything else this game says out loud.
    pub fn line(self, day: u32) -> String {
        if self.was_good() {
            let supper = match self.meal {
                Meal::Bowl => "There was a bowl in the pot and wood in the fire.",
                Meal::Raw => "You ate out of the pack, which is not the same thing.",
                Meal::Half => "Half a bowl, which you had already decided about.",
                _ => "",
            };
            return format!("Day {day}. Fed, warm, and dry. {supper}")
                .trim_end()
                .to_owned();
        }
        let mut wrong = Vec::new();
        if !self.fed {
            wrong.push(match self.meal {
                Meal::HotWater => "hot water and nothing in it",
                _ => "nothing to eat",
            });
        }
        if self.fire_went_out {
            wrong.push("the fire burnt out in the night");
        } else if !self.warm {
            wrong.push("a cold hearth");
        }
        if !self.dry {
            wrong.push("a night spent up rather than under a roof");
        }
        format!("Day {day}. {}.", sentence(&wrong))
    }
}

/// Joins the night's complaints the way a person would say them.
fn sentence(parts: &[&str]) -> String {
    match parts {
        [] => "Nothing to report".to_owned(),
        [one] => capitalise(one),
        [rest @ .., last] => format!("{}, and {last}", capitalise(&rest.join(", "))),
    }
}

fn capitalise(text: &str) -> String {
    let mut characters = text.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + characters.as_str()
    })
}

/// Everything the waystation has to keep paying for.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Resource, Serialize)]
pub struct Upkeep {
    pot: Stew,
    /// Nights of fire banked in the hearth.
    banked: u8,
    /// Nights running with nothing to eat. Splitting a bowl does not clear it.
    hungry_nights: u16,
    good_days: u32,
    /// Set by the bed and spent by the settlement, which is the whole of what
    /// makes a night dry.
    slept: bool,
    /// Half a bowl kept back after sharing the last of it.
    kept_half: bool,
}

impl Upkeep {
    pub const fn pot(&self) -> &Stew {
        &self.pot
    }

    pub const fn banked_nights(&self) -> u8 {
        self.banked
    }

    /// Lighting the hearth lays the first night's wood with it.
    pub const fn lay_a_fire(&mut self) {
        if self.banked == 0 {
            self.banked = 1;
        }
    }

    /// A bed, and therefore a dry night, once.
    pub const fn slept_here(&mut self) {
        self.slept = true;
    }

    pub const fn fire_wants_wood(&self) -> bool {
        self.banked < MAX_BANKED_NIGHTS
    }

    /// What the Scribe would put on the fire if asked to right now: a split log
    /// first, because kindling is what relights a hearth that has gone out and
    /// burning it for warmth is spending the matches.
    pub fn wood_on_hand(&self, progression: &Progression) -> Option<Fuel> {
        if !self.fire_wants_wood() {
            return None;
        }
        if progression.supply(SupplyId::Log) > 0 {
            Some(Fuel::Log)
        } else if progression.supply(SupplyId::Kindling) >= KINDLING_PER_FEED {
            Some(Fuel::Kindling)
        } else {
            None
        }
    }

    /// Puts wood on. Returns what went on, or nothing if there was none to put.
    ///
    /// `sheltered` is the office with every wall section mended. Nothing says so
    /// out loud; the fire simply lasts a night longer on the same armful, which
    /// is the only lasting dividend a repair pays in this game.
    pub fn feed_the_fire(
        &mut self,
        progression: &mut Progression,
        sheltered: bool,
    ) -> Option<Fuel> {
        let fuel = self.wood_on_hand(progression)?;
        let extra = u8::from(sheltered);
        let nights = match fuel {
            Fuel::Log => {
                progression.spend_supply(SupplyId::Log, 1);
                NIGHTS_PER_LOG + extra
            }
            Fuel::Kindling => {
                progression.spend_supply(SupplyId::Kindling, KINDLING_PER_FEED);
                1 + extra
            }
        };
        self.banked = (self.banked + nights).min(MAX_BANKED_NIGHTS);
        Some(fuel)
    }

    pub fn can_add_a_ration(&self, progression: &Progression) -> bool {
        self.pot.has_room_for_a_ration() && progression.supply(SupplyId::Ration) > 0
    }

    pub fn add_a_ration(&mut self, progression: &mut Progression) -> bool {
        if !self.can_add_a_ration(progression) {
            return false;
        }
        progression.spend_supply(SupplyId::Ration, 1);
        self.pot.add_a_ration();
        true
    }

    pub fn can_stretch(&self, progression: &Progression) -> bool {
        self.pot.worth_stretching() && progression.supply(SupplyId::Water) > 0
    }

    pub fn stretch_the_pot(&mut self, progression: &mut Progression) -> bool {
        if !self.can_stretch(progression) {
            return false;
        }
        progression.spend_supply(SupplyId::Water, 1);
        self.pot.stretch();
        true
    }

    /// Whether there is a bowl to give away and still one to eat afterwards.
    pub const fn can_ladle_a_bowl(&self) -> bool {
        self.pot.bowls > 1 && self.pot.is_food()
    }

    /// Whether what is left has to be halved to be shared at all.
    pub const fn only_the_last_bowl(&self) -> bool {
        self.pot.bowls == 1 && self.pot.is_food()
    }

    /// A bowl out of the pot for somebody else. Returns its strength.
    pub const fn ladle_a_bowl(&mut self) -> u16 {
        self.pot.take_a_bowl()
    }

    /// The last of it, down the middle. The bowl leaves the pot, the stranger
    /// eats, and the Scribe keeps a half that will not settle anything.
    pub const fn share_the_last(&mut self) -> u16 {
        self.kept_half = true;
        self.pot.take_a_bowl()
    }

    /// The same decision when there is no pot to speak of: one ration, halved.
    pub fn split_a_ration(&mut self, progression: &mut Progression) -> bool {
        if progression.supply(SupplyId::Ration) == 0 {
            return false;
        }
        progression.spend_supply(SupplyId::Ration, 1);
        self.kept_half = true;
        true
    }

    /// Settles the night. Burns a night of wood, feeds the Scribe out of
    /// whatever there is, and reports what kind of night it was.
    pub fn settle_night(&mut self, progression: &mut Progression, fire_was_lit: bool) -> Night {
        let warm = fire_was_lit && self.banked > 0;
        if warm {
            self.banked -= 1;
        }
        let meal = self.eat(progression);
        let fed = matches!(meal, Meal::Bowl | Meal::Raw | Meal::Half);
        let night = Night {
            fed,
            warm,
            dry: self.slept,
            meal,
            fire_went_out: fire_was_lit && !warm,
        };
        self.slept = false;
        if fed && meal != Meal::Half {
            self.hungry_nights = 0;
        } else if !fed {
            self.hungry_nights += 1;
        }
        if night.was_good() {
            self.good_days += 1;
        }
        night
    }

    /// Supper, in the order a person would actually reach for it. The half kept
    /// back from sharing is spent whether or not it gets eaten: a Scribe who
    /// filled the pot again before dark has a proper supper and the decision
    /// they made at noon costs them nothing further.
    fn eat(&mut self, progression: &mut Progression) -> Meal {
        let kept_half = std::mem::take(&mut self.kept_half);
        if self.pot.is_food() {
            self.pot.take_a_bowl();
            return Meal::Bowl;
        }
        if kept_half {
            return Meal::Half;
        }
        if progression.spend_supply(SupplyId::Ration, 1) {
            return Meal::Raw;
        }
        if self.pot.bowls() > 0 {
            self.pot.take_a_bowl();
            return Meal::HotWater;
        }
        Meal::Nothing
    }

    /// The hearth as the corner of the screen reports it: how long the fire will
    /// hold, and what is in the pot.
    pub fn summary(&self, fire_is_lit: bool) -> String {
        let fire = if fire_is_lit {
            match self.banked {
                0 => "burning down to embers".to_owned(),
                1 => "wood in for tonight".to_owned(),
                nights => format!("wood in for {nights} nights"),
            }
        } else {
            "cold".to_owned()
        };
        format!("{fire} · {}", self.pot.describe())
    }

    /// The two things worth keeping a running count of. Good days are the only
    /// score this game keeps, and it does not keep it where it can be aimed at:
    /// hungry nights sit on the same line, unexplained.
    pub fn tally(&self) -> Option<String> {
        let mut counted = Vec::new();
        match self.good_days {
            0 => {}
            1 => counted.push("one good day".to_owned()),
            days => counted.push(format!("{days} good days")),
        }
        match self.hungry_nights {
            0 => {}
            1 => counted.push("one night hungry".to_owned()),
            nights => counted.push(format!("{nights} nights hungry")),
        }
        (!counted.is_empty()).then(|| counted.join(" · "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stocked() -> (Upkeep, Progression) {
        let mut upkeep = Upkeep::default();
        let mut progression = Progression::default();
        progression.add_supply(SupplyId::Log, 4);
        progression.add_supply(SupplyId::Ration, 4);
        progression.add_supply(SupplyId::Water, 4);
        upkeep.lay_a_fire();
        (upkeep, progression)
    }

    #[test]
    fn a_lit_hearth_burns_a_night_of_wood_and_then_goes_out() {
        let (mut upkeep, mut progression) = stocked();
        // Lighting it laid one night's wood, and no more.
        let first = upkeep.settle_night(&mut progression, true);
        assert!(first.warm);
        assert!(!first.fire_went_out);

        let second = upkeep.settle_night(&mut progression, true);
        assert!(!second.warm);
        assert!(second.fire_went_out, "an unfed fire does not last a week");
    }

    #[test]
    fn a_split_log_is_two_nights_and_the_hearth_will_not_hold_a_winter() {
        let (mut upkeep, mut progression) = stocked();
        assert_eq!(
            upkeep.feed_the_fire(&mut progression, false),
            Some(Fuel::Log)
        );
        assert_eq!(upkeep.banked_nights(), 3);
        for _ in 0..4 {
            upkeep.feed_the_fire(&mut progression, false);
        }
        assert_eq!(upkeep.banked_nights(), MAX_BANKED_NIGHTS);
        assert!(
            upkeep.wood_on_hand(&progression).is_none(),
            "a full hearth does not offer to take more"
        );
    }

    /// The only lasting dividend a repair pays. Mending every wall in the office
    /// does not announce itself; the same armful simply lasts a night longer.
    #[test]
    fn a_room_that_holds_its_heat_makes_the_woodpile_go_further() {
        let mut draughty = Upkeep::default();
        let mut sound = Upkeep::default();
        let mut progression = Progression::default();
        progression.add_supply(SupplyId::Log, 2);

        draughty.feed_the_fire(&mut progression, false);
        sound.feed_the_fire(&mut progression, true);
        assert_eq!(draughty.banked_nights(), NIGHTS_PER_LOG);
        assert_eq!(sound.banked_nights(), NIGHTS_PER_LOG + 1);
        assert_eq!(progression.supply(SupplyId::Log), 0, "one log either way");
    }

    /// Kindling is what relights a hearth, so it is the second choice for
    /// warming one — the log goes on while there is a log.
    #[test]
    fn the_fire_takes_a_log_before_it_takes_the_kindling() {
        let mut upkeep = Upkeep::default();
        let mut progression = Progression::default();
        progression.add_supply(SupplyId::Log, 1);
        progression.add_supply(SupplyId::Kindling, KINDLING_PER_FEED);

        assert_eq!(
            upkeep.feed_the_fire(&mut progression, false),
            Some(Fuel::Log)
        );
        assert_eq!(progression.supply(SupplyId::Kindling), KINDLING_PER_FEED);
        assert_eq!(
            upkeep.feed_the_fire(&mut progression, false),
            Some(Fuel::Kindling)
        );
        assert_eq!(progression.supply(SupplyId::Kindling), 0);
    }

    #[test]
    fn a_ration_in_the_pot_is_two_bowls_and_water_makes_it_more_and_worse() {
        let (mut upkeep, mut progression) = stocked();
        assert!(upkeep.add_a_ration(&mut progression));
        assert_eq!(upkeep.pot().bowls(), 2);
        assert_eq!(upkeep.pot().strength(), FULL);

        assert!(upkeep.stretch_the_pot(&mut progression));
        assert!(upkeep.stretch_the_pot(&mut progression));
        assert_eq!(upkeep.pot().bowls(), 4);
        assert_eq!(upkeep.pot().strength(), FULL / 2);
        assert!(upkeep.pot().is_food(), "half strength still feeds somebody");
    }

    /// The limit on stretching is not a rule the game states; it is that at some
    /// point the pot stops being food and the Scribe can taste it.
    #[test]
    fn a_pot_let_down_far_enough_stops_being_supper() {
        let mut upkeep = Upkeep::default();
        let mut progression = Progression::default();
        progression.add_supply(SupplyId::Ration, 1);
        progression.add_supply(SupplyId::Water, 20);
        upkeep.add_a_ration(&mut progression);

        while upkeep.stretch_the_pot(&mut progression) {}
        assert!(
            !upkeep.can_stretch(&progression),
            "the game stops offering before the pot is water"
        );
        assert!(upkeep.pot().is_food(), "and stops while it is still food");
        assert!(upkeep.pot().bowls() >= 6, "one ration went a long way");
    }

    #[test]
    fn the_scribe_eats_the_pot_first_and_the_pack_only_when_it_is_empty() {
        let (mut upkeep, mut progression) = stocked();
        upkeep.add_a_ration(&mut progression);
        let rations = progression.supply(SupplyId::Ration);

        assert_eq!(upkeep.settle_night(&mut progression, true).meal, Meal::Bowl);
        assert_eq!(progression.supply(SupplyId::Ration), rations);
        upkeep.feed_the_fire(&mut progression, false);
        assert_eq!(upkeep.settle_night(&mut progression, true).meal, Meal::Bowl);
        // Pot empty now: the pack is what is left.
        assert_eq!(upkeep.settle_night(&mut progression, true).meal, Meal::Raw);
        assert_eq!(progression.supply(SupplyId::Ration), rations - 1);
    }

    #[test]
    fn nothing_to_eat_is_a_hungry_night_that_keeps_count() {
        let mut upkeep = Upkeep::default();
        let mut progression = Progression::default();

        let night = upkeep.settle_night(&mut progression, false);
        assert!(!night.fed);
        assert!(!night.warm);
        assert_eq!(night.meal, Meal::Nothing);
        assert_eq!(upkeep.hungry_nights, 1);
        upkeep.settle_night(&mut progression, false);
        assert_eq!(upkeep.hungry_nights, 2);

        progression.add_supply(SupplyId::Ration, 1);
        upkeep.settle_night(&mut progression, false);
        assert_eq!(upkeep.hungry_nights, 0, "one supper settles the count");
    }

    /// The whole point of the split: both people eat, and neither of them has
    /// eaten. A half does not end a hungry stretch.
    #[test]
    fn sharing_the_last_bowl_down_the_middle_feeds_neither_properly() {
        let (mut upkeep, mut progression) = stocked();
        upkeep.add_a_ration(&mut progression);
        upkeep.ladle_a_bowl();
        assert!(upkeep.only_the_last_bowl());
        assert!(!upkeep.can_ladle_a_bowl(), "one bowl cannot be given whole");

        upkeep.hungry_nights = 3;
        upkeep.share_the_last();
        assert_eq!(upkeep.pot().bowls(), 0, "the last of it is the last of it");

        let night = upkeep.settle_night(&mut progression, true);
        assert_eq!(night.meal, Meal::Half);
        assert!(night.fed, "half a bowl is still supper");
        assert_eq!(upkeep.hungry_nights, 3, "but it does not settle anything");
    }

    /// Sharing at noon and refilling the pot before dark is a proper supper.
    /// The half is spent either way; it is not a debt that follows the Scribe.
    #[test]
    fn a_pot_filled_again_before_dark_beats_the_half_that_was_kept() {
        let (mut upkeep, mut progression) = stocked();
        upkeep.add_a_ration(&mut progression);
        upkeep.ladle_a_bowl();
        upkeep.share_the_last();
        upkeep.add_a_ration(&mut progression);

        let night = upkeep.settle_night(&mut progression, true);
        assert_eq!(night.meal, Meal::Bowl);
        assert_eq!(upkeep.hungry_nights, 0);
        assert!(!upkeep.kept_half, "the half does not carry over");
    }

    #[test]
    fn a_night_spent_standing_up_is_not_a_dry_one() {
        let (mut upkeep, mut progression) = stocked();
        upkeep.add_a_ration(&mut progression);
        assert!(!upkeep.settle_night(&mut progression, true).dry);

        upkeep.feed_the_fire(&mut progression, false);
        upkeep.slept_here();
        let night = upkeep.settle_night(&mut progression, true);
        assert!(night.was_good());
        assert_eq!(upkeep.good_days, 1);
        assert!(
            !upkeep.settle_night(&mut progression, true).dry,
            "once each"
        );
    }

    #[test]
    fn the_morning_line_names_everything_that_was_wrong_and_nothing_else() {
        let good = Night {
            fed: true,
            warm: true,
            dry: true,
            meal: Meal::Bowl,
            fire_went_out: false,
        };
        assert!(good.line(9).starts_with("Day 9. Fed, warm, and dry."));

        let bad = Night {
            fed: false,
            warm: false,
            dry: false,
            meal: Meal::Nothing,
            fire_went_out: true,
        };
        assert_eq!(
            bad.line(9),
            "Day 9. Nothing to eat, the fire burnt out in the night, and a night spent up rather than under a roof."
        );

        let hungry = Night {
            fed: false,
            warm: true,
            dry: true,
            meal: Meal::HotWater,
            fire_went_out: false,
        };
        assert_eq!(hungry.line(9), "Day 9. Hot water and nothing in it.");
    }

    #[test]
    fn the_hearth_reports_itself_without_telling_anybody_what_to_do() {
        let (mut upkeep, mut progression) = stocked();
        assert_eq!(
            upkeep.summary(false),
            "cold · the pot is empty",
            "an unlit hearth is not counting down"
        );
        upkeep.feed_the_fire(&mut progression, false);
        upkeep.add_a_ration(&mut progression);
        assert_eq!(
            upkeep.summary(true),
            "wood in for 3 nights · 2 bowls, thick"
        );
        upkeep.settle_night(&mut progression, true);
        upkeep.settle_night(&mut progression, true);
        upkeep.settle_night(&mut progression, true);
        assert_eq!(
            upkeep.summary(true),
            "burning down to embers · the pot is empty"
        );
    }

    #[test]
    fn the_pot_and_the_woodpile_survive_a_save() {
        let (mut upkeep, mut progression) = stocked();
        upkeep.feed_the_fire(&mut progression, false);
        upkeep.add_a_ration(&mut progression);
        upkeep.stretch_the_pot(&mut progression);
        let raw = serde_json::to_string(&upkeep).expect("serialize upkeep");
        let restored: Upkeep = serde_json::from_str(&raw).expect("restore upkeep");
        assert_eq!(restored, upkeep);
    }
}
