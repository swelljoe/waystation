//! The valley's improvised garden.
//!
//! Every other restoration in the motel is repair: something existed, it broke,
//! the Scribe puts it back. The garden is the one loop that makes something the
//! valley did not have before, and it is deliberately the slowest. A plot is
//! carried by hand through six states — cracked parking bay, broken ground,
//! tilled rows, sown seed, standing grain, harvest — and each state names
//! exactly one thing that has to happen next. Only the growing is out of the
//! Scribe's hands.
//!
//! The beds are the motel's own parking spaces. Under the asphalt is soil from
//! before the ash, which is the only ground in the valley that will carry more
//! than straggly weeds, and getting at it means levering up the slabs.

use std::collections::BTreeMap;

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::chance::Chance;
use crate::daylight::DAY_SECONDS;
use crate::progression::TaskSpec;

/// How long standing grain takes to fill out: watered this morning, standing
/// the next. Grain is the one thing here the Scribe cannot hurry, and a season
/// short enough to watch through was a season short enough to farm in circles.
/// Sleeping skips the rest of a night without advancing the beds, so a player
/// who sleeps waits longer than this rather than less.
pub const RIPEN_SECONDS: f32 = DAY_SECONDS;
/// The valley is generous more often than the wastes ever were, but not on
/// schedule. Roughly one harvest in twelve comes up heavier than promised —
/// drawn each time rather than counted out, so that nine working beds do not
/// deliver a windfall every season like clockwork.
const GENEROUS_HARVEST_ODDS: f32 = 1.0 / 12.0;
const GENEROUS_HARVEST_BONUS: u16 = 2;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlotStage {
    /// A cracked parking bay, weeds in the seams, soil from before underneath.
    #[default]
    Paved,
    Fallow,
    Tilled,
    Sown,
    Growing,
    Ripe,
}

impl PlotStage {
    /// The art for a bed that is being worked. `Paved` and `Fallow` are not
    /// here: those are the authored repair pair's own two states, so the lot can
    /// be laid out and re-skinned in the editor without touching this file.
    pub const fn grown_art(self, nearly_ripe: bool) -> Option<&'static str> {
        match self {
            Self::Paved | Self::Fallow => None,
            Self::Tilled => Some("world/garden_plot_tilled.png"),
            Self::Sown => Some("world/garden_plot_sown.png"),
            Self::Growing => Some(if nearly_ripe {
                "world/garden_plot_growing.png"
            } else {
                "world/garden_plot_sprouting.png"
            }),
            Self::Ripe => Some("world/garden_plot_ripe.png"),
        }
    }

    /// What the Scribe would be doing here, in the prompt's own words.
    pub const fn work(self) -> &'static str {
        match self {
            Self::Paved => "tear out the broken asphalt",
            Self::Fallow => "till this ground into rows",
            Self::Tilled => "sow a seed here",
            Self::Sown => "water the sown seed",
            Self::Growing => "let the grain come up",
            Self::Ripe => "harvest the grain",
        }
    }

    /// The work this state is waiting for, or `None` while the plot is only
    /// waiting on time. `Paved` is the exception: what it takes to lever a slab
    /// up is authored on the repair pair, so the caller supplies it.
    pub fn task(self) -> Option<TaskSpec> {
        match self {
            // Levering a slab up is authored; growing waits on the clock alone.
            Self::Paved | Self::Growing => None,
            Self::Fallow => Some(TaskSpec::for_tilling()),
            Self::Tilled => Some(TaskSpec::for_sowing()),
            Self::Sown => Some(TaskSpec::for_watering()),
            Self::Ripe => Some(TaskSpec::for_harvest()),
        }
    }

    pub const fn is_paved(self) -> bool {
        matches!(self, Self::Paved)
    }

    /// Where the plot stands once that work is done. Torn-out asphalt and
    /// harvested stubble arrive at the same place: a slab comes up only once, so
    /// the second season and every one after it starts from open ground.
    const fn next(self) -> Self {
        match self {
            Self::Paved | Self::Ripe => Self::Fallow,
            Self::Fallow => Self::Tilled,
            Self::Tilled => Self::Sown,
            Self::Sown => Self::Growing,
            Self::Growing => Self::Ripe,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Plot {
    stage: PlotStage,
    /// Seconds of growing still owed. Saved, so a season does not restart with
    /// the browser tab.
    #[serde(default)]
    ripening_seconds: f32,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Resource, Serialize)]
pub struct Garden {
    #[serde(default)]
    plots: BTreeMap<String, Plot>,
}

/// What one finished piece of garden work produced, for the line the Scribe
/// reads afterwards.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Worked {
    pub from: PlotStage,
    pub to: PlotStage,
    /// Rations beyond what the task promised, on the harvests that surprise.
    pub bonus_rations: u16,
}

impl Garden {
    pub fn stage(&self, plot_id: &str) -> PlotStage {
        self.plots
            .get(plot_id)
            .map_or(PlotStage::Paved, |plot| plot.stage)
    }

    /// True once the grain is closer to ripe than to sown, which is the only
    /// thing the waiting player can be told without inventing a clock.
    pub fn nearly_ripe(&self, plot_id: &str) -> bool {
        self.plots.get(plot_id).is_some_and(|plot| {
            plot.stage == PlotStage::Growing && plot.ripening_seconds < RIPEN_SECONDS / 2.0
        })
    }

    /// Advances the plot one state. The caller has already paid for it; this
    /// only records what changed — and, on a harvest, draws for whether the bed
    /// gave more than it was asked for.
    pub fn advance(&mut self, plot_id: &str, chance: &mut Chance) -> Worked {
        let plot = self.plots.entry(plot_id.to_owned()).or_default();
        let from = plot.stage;
        let to = from.next();
        plot.stage = to;
        if to == PlotStage::Growing {
            plot.ripening_seconds = RIPEN_SECONDS;
        }
        // Drawn at the harvest itself rather than counted towards it, so that no
        // run of lean seasons is owed a good one.
        let bonus_rations = if from == PlotStage::Ripe && chance.odds(GENEROUS_HARVEST_ODDS) {
            GENEROUS_HARVEST_BONUS
        } else {
            0
        };
        Worked {
            from,
            to,
            bonus_rations,
        }
    }

    /// Runs the growing clock and reports the plots that came ripe this tick,
    /// so the world can say so rather than letting the player discover it by
    /// walking past.
    pub fn tick(&mut self, seconds: f32) -> Vec<String> {
        let mut ripened = Vec::new();
        for (id, plot) in &mut self.plots {
            if plot.stage != PlotStage::Growing {
                continue;
            }
            plot.ripening_seconds = (plot.ripening_seconds - seconds).max(0.0);
            if plot.ripening_seconds <= 0.0 {
                plot.stage = PlotStage::Ripe;
                ripened.push(id.clone());
            }
        }
        ripened
    }

    pub fn is_growing(&self) -> bool {
        self.plots
            .values()
            .any(|plot| plot.stage == PlotStage::Growing)
    }

    /// Bays whose asphalt has come up at least once, which is the only garden
    /// progress worth reporting outside the garden itself.
    pub fn broken_ground(&self) -> usize {
        self.plots
            .values()
            .filter(|plot| plot.stage != PlotStage::Paved)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::progression::{Progression, SkillId, SupplyId, TaskSpec, ToolId};

    fn ready_gardener() -> Progression {
        let mut progression = Progression::default();
        for _ in 0..3 {
            progression
                .attempt(&TaskSpec::for_kind("debris"))
                .expect("cleaning has no requirements");
        }
        progression.add_tool(ToolId::Pickaxe);
        progression.add_tool(ToolId::Hoe);
        progression.add_tool(ToolId::WateringCan);
        progression
    }

    /// The whole garden, walked once: asphalt to rations, with the water drawn
    /// the way the Scribe has to draw it.
    #[test]
    fn one_bay_carries_asphalt_through_to_rations() {
        let mut garden = Garden::default();
        let mut chance = Chance::default();
        let mut progression = ready_gardener();
        progression.add_supply(SupplyId::Seed, 1);
        progression
            .attempt(&TaskSpec::for_drawing_water())
            .expect("a can at the river");

        // The authored pair supplies the levering-up task; everything after it
        // is the garden's own.
        progression
            .attempt(&TaskSpec::for_breaking_ground())
            .expect("a pickaxe and a slab");
        garden.advance("plot-00", &mut chance);
        while garden.stage("plot-00") != PlotStage::Ripe {
            let stage = garden.stage("plot-00");
            match stage.task() {
                Some(task) => {
                    progression
                        .attempt(&task)
                        .unwrap_or_else(|reason| panic!("{stage:?} should be payable: {reason}"));
                    garden.advance("plot-00", &mut chance);
                }
                // Only the growing is out of the Scribe's hands.
                None => assert_eq!(garden.tick(RIPEN_SECONDS), vec!["plot-00".to_owned()]),
            }
        }

        progression
            .attempt(&TaskSpec::for_harvest())
            .expect("ripe grain asks for nothing");
        garden.advance("plot-00", &mut chance);

        assert_eq!(garden.stage("plot-00"), PlotStage::Fallow);
        assert_eq!(progression.supply(SupplyId::Ration), 3);
        // One seed went in and two came back: the garden can widen next season.
        assert_eq!(progression.supply(SupplyId::Seed), 2);
        assert_eq!(progression.supply(SupplyId::Water), 2);
        assert_eq!(progression.skill_level(SkillId::Cultivation), 1);
    }

    #[test]
    fn a_slab_comes_up_once_and_the_ground_stays_open() {
        let mut garden = Garden::default();
        let mut chance = Chance::default();
        assert_eq!(garden.stage("plot-00"), PlotStage::Paved);
        for _ in 0..5 {
            garden.advance("plot-00", &mut chance);
        }

        assert_eq!(garden.stage("plot-00"), PlotStage::Ripe);
        assert_eq!(garden.advance("plot-00", &mut chance).to, PlotStage::Fallow);
        assert_eq!(garden.broken_ground(), 1);
    }

    #[test]
    fn grain_ripens_on_its_own_clock_and_says_so_once() {
        let mut garden = Garden::default();
        let mut chance = Chance::default();
        for _ in 0..4 {
            garden.advance("plot-00", &mut chance);
        }
        assert_eq!(garden.stage("plot-00"), PlotStage::Growing);
        assert!(!garden.nearly_ripe("plot-00"));

        assert!(garden.tick(RIPEN_SECONDS * 0.6).is_empty());
        assert!(garden.nearly_ripe("plot-00"));
        assert_eq!(
            garden.tick(RIPEN_SECONDS),
            vec!["plot-00".to_owned()],
            "a plot announces its own ripening exactly once"
        );
        assert!(garden.tick(RIPEN_SECONDS).is_empty());
        assert_eq!(garden.stage("plot-00"), PlotStage::Ripe);
    }

    /// A season nobody can outwait is a season nobody plans around. Grain
    /// watered during one day's work is not standing until the next.
    #[test]
    fn watered_grain_is_not_ready_the_same_day() {
        let mut garden = Garden::default();
        let mut chance = Chance::default();
        for _ in 0..4 {
            garden.advance("plot-00", &mut chance);
        }

        assert_eq!(garden.stage("plot-00"), PlotStage::Growing);
        assert!(
            garden.tick(DAY_SECONDS - 1.0).is_empty(),
            "a bed came ripe inside the day it was watered"
        );
        assert_eq!(garden.tick(1.0), vec!["plot-00".to_owned()]);
    }

    /// Carries one bed round to its next harvest and reports what that harvest
    /// gave beyond the promise.
    fn harvest_once(garden: &mut Garden, chance: &mut Chance, plot: &str) -> u16 {
        while garden.stage(plot) != PlotStage::Ripe {
            if garden.stage(plot) == PlotStage::Growing {
                garden.tick(RIPEN_SECONDS);
            } else {
                garden.advance(plot, chance);
            }
        }
        garden.advance(plot, chance).bonus_rations
    }

    /// Hope in this valley is not a schedule. The promised yield is always
    /// paid; the extra is never advertised, arrives about one harvest in
    /// twelve, and arrives on no count a player could learn to plant around.
    #[test]
    fn a_generous_harvest_is_drawn_rather_than_counted_out() {
        let mut garden = Garden::default();
        let mut chance = Chance::default();
        let mut gaps = Vec::new();
        let mut since = 0;
        let seasons = 1200;
        for _ in 0..seasons {
            since += 1;
            if harvest_once(&mut garden, &mut chance, "plot-00") > 0 {
                gaps.push(since);
                since = 0;
            }
        }

        let generous = gaps.len();
        let expected = seasons / 12;
        assert!(
            (expected * 3 / 4..=expected * 5 / 4).contains(&generous),
            "{generous} generous harvests in {seasons}, wanted about {expected}"
        );
        // Nine beds working at once would hand out a windfall every season if
        // the extra came round on a fixed count, so it must not.
        assert!(
            gaps.iter().collect::<BTreeSet<_>>().len() > 15,
            "the gaps between generous harvests look like a schedule: {gaps:?}"
        );
        assert!(!TaskSpec::for_harvest().yields_text().contains('4'));
    }

    #[test]
    fn a_growing_season_survives_being_saved_and_reloaded() {
        let mut garden = Garden::default();
        let mut chance = Chance::default();
        for _ in 0..4 {
            garden.advance("plot-00", &mut chance);
        }
        garden.tick(RIPEN_SECONDS * 0.75);

        let raw = serde_json::to_string(&garden).expect("serialize garden");
        let mut restored: Garden = serde_json::from_str(&raw).expect("restore garden");

        assert_eq!(restored.stage("plot-00"), PlotStage::Growing);
        assert!(restored.nearly_ripe("plot-00"));
        assert!(restored.tick(RIPEN_SECONDS * 0.20).is_empty());
        assert_eq!(restored.tick(RIPEN_SECONDS), vec!["plot-00".to_owned()]);
    }
}
