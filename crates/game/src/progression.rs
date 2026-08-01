//! Small, data-driven restoration progression for the motel repair loop.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

const XP_PER_LEVEL: u16 = 3;
const MAX_SKILL_LEVEL: u8 = 3;
pub const MAX_CARRIED_TOOLS: usize = 3;

/// Mending a tool is not a formality. A hand that has only ever swept debris
/// does not know what a sound joint feels like, so the work waits on a season
/// of easier repairs first.
const TOOL_REPAIR_LEVEL: u8 = 2;

/// Total experience in one skill from nothing to its ceiling.
#[cfg_attr(not(test), allow(dead_code))]
pub const fn xp_for_max_level() -> u32 {
    XP_PER_LEVEL as u32 * MAX_SKILL_LEVEL as u32
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillId {
    Upkeep,
    Carpentry,
    Masonry,
    Roofing,
    Cultivation,
}

impl SkillId {
    pub const ALL: [Self; 5] = [
        Self::Upkeep,
        Self::Carpentry,
        Self::Masonry,
        Self::Roofing,
        Self::Cultivation,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Upkeep => "Upkeep",
            Self::Carpentry => "Carpentry",
            Self::Masonry => "Masonry",
            Self::Roofing => "Roofing",
            Self::Cultivation => "Cultivation",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolId {
    Hammer,
    Hatchet,
    Trowel,
    Ladder,
    Pickaxe,
    Shovel,
    Hoe,
    WateringCan,
}

impl ToolId {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Hammer => "hammer",
            Self::Hatchet => "hatchet",
            Self::Trowel => "trowel",
            Self::Ladder => "ladder",
            Self::Pickaxe => "pickaxe",
            Self::Shovel => "shovel",
            Self::Hoe => "hoe",
            Self::WateringCan => "watering can",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCondition {
    Broken,
    #[default]
    Serviceable,
}

impl ToolCondition {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Broken => "broken",
            Self::Serviceable => "serviceable",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "place", rename_all = "snake_case")]
pub enum ToolLocation {
    #[default]
    Home,
    Carried,
    Dropped {
        scene_id: String,
        x: i32,
        y: i32,
    },
    HeldBy {
        actor_id: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PortableToolRecord {
    pub tool: ToolId,
    pub condition: ToolCondition,
    #[serde(default)]
    pub location: ToolLocation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupplyId {
    Kindling,
    Log,
    Plank,
    Nails,
    Stone,
    Cloth,
    Seed,
    Water,
    Ration,
}

impl SupplyId {
    pub const ALL: [Self; 9] = [
        Self::Kindling,
        Self::Log,
        Self::Plank,
        Self::Nails,
        Self::Stone,
        Self::Cloth,
        Self::Seed,
        Self::Water,
        Self::Ration,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Kindling => "kindling",
            Self::Log => "fallen logs",
            Self::Plank => "sound planks",
            Self::Nails => "nails",
            Self::Stone => "stone",
            Self::Cloth => "cloth",
            Self::Seed => "seeds",
            Self::Water => "canfuls of water",
            Self::Ration => "rations",
        }
    }

    /// Requirement lines read as sentences, so a single board is a board.
    pub const fn label_for(self, amount: u16) -> &'static str {
        if amount == 1 {
            match self {
                Self::Log => "fallen log",
                Self::Plank => "sound plank",
                Self::Nails => "nail",
                Self::Seed => "seed",
                Self::Water => "canful of water",
                Self::Ration => "ration",
                _ => self.label(),
            }
        } else {
            self.label()
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskAction {
    Clean,
    Repair,
    Clear,
    Restore,
    Light,
    Mill,
    Quarry,
    Break,
    Till,
    Sow,
    Water,
    Harvest,
    Draw,
}

impl TaskAction {
    pub const fn infinitive(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Repair => "repair",
            Self::Clear => "clear",
            Self::Restore => "restore",
            Self::Light => "light",
            Self::Mill => "mill",
            Self::Quarry => "quarry",
            Self::Break => "break",
            Self::Till => "till",
            Self::Sow => "sow",
            Self::Water => "water",
            Self::Harvest => "harvest",
            Self::Draw => "draw",
        }
    }

    pub const fn past_tense(self) -> &'static str {
        match self {
            Self::Clean => "cleaned",
            Self::Repair => "repaired",
            Self::Clear => "cleared",
            Self::Restore => "restored",
            Self::Light => "lit",
            Self::Mill => "milled",
            Self::Quarry => "quarried",
            Self::Break => "broken",
            Self::Till => "tilled",
            Self::Sow => "sown",
            Self::Water => "watered",
            Self::Harvest => "harvested",
            Self::Draw => "drawn",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SupplyCost {
    pub item: SupplyId,
    pub amount: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TaskSpec {
    pub action: TaskAction,
    pub skill: SkillId,
    #[serde(default)]
    pub level: u8,
    #[serde(default)]
    pub tools: Vec<ToolId>,
    #[serde(default)]
    pub supplies: Vec<SupplyCost>,
    /// Materials where any one option will do. A broken haft wants wood, and
    /// the valley does not care whether it comes off a fallen log or out of a
    /// sound plank. Empty means the job offers no such choice.
    #[serde(default)]
    pub any_of: Vec<SupplyCost>,
    /// Materials the work gives back: nails pulled from cleared debris, planks
    /// cut from a log, stone broken out of an outcrop. This is what keeps the
    /// restoration economy circulating instead of draining one-time props.
    #[serde(default)]
    pub yields: Vec<SupplyCost>,
    #[serde(default = "default_xp")]
    pub xp: u16,
}

const fn default_xp() -> u16 {
    1
}

impl TaskSpec {
    /// Compatibility defaults keep older repair pairs playable until their
    /// more specific requirements are authored in the editor.
    pub fn for_kind(kind: &str) -> Self {
        match kind {
            "debris" => {
                Self::new(TaskAction::Clean, SkillId::Upkeep, 0).with_yield(SupplyId::Nails, 1)
            }
            "floor" => Self::new(TaskAction::Repair, SkillId::Carpentry, 0)
                .with_tools(&[ToolId::Hammer])
                .with_supply(SupplyId::Plank, 1),
            "furniture" | "furnitute" | "bed" | "nightstand" => {
                Self::new(TaskAction::Repair, SkillId::Carpentry, 1)
                    .with_tools(&[ToolId::Hammer])
                    .with_supply(SupplyId::Plank, 1)
            }
            // Mortar and rubble are shovel work; no trowel survives in the valley.
            "wall" | "fireplace" => Self::new(TaskAction::Repair, SkillId::Masonry, 0)
                .with_tools(&[ToolId::Shovel])
                .with_supply(SupplyId::Stone, 1),
            "chimney" => {
                Self::new(TaskAction::Clear, SkillId::Upkeep, 1).with_tools(&[ToolId::Ladder])
            }
            "roof" => Self::new(TaskAction::Repair, SkillId::Roofing, 0)
                .with_tools(&[ToolId::Hammer, ToolId::Ladder])
                .with_supply(SupplyId::Plank, 1),
            "door" | "window" | "mirror" | "lamp" => {
                Self::new(TaskAction::Repair, SkillId::Upkeep, 1)
                    .with_tools(&[ToolId::Hammer])
                    .with_supply(SupplyId::Nails, 1)
            }
            _ => Self::new(TaskAction::Repair, SkillId::Upkeep, 1),
        }
    }

    const fn new(action: TaskAction, skill: SkillId, level: u8) -> Self {
        Self {
            action,
            skill,
            level,
            tools: Vec::new(),
            supplies: Vec::new(),
            any_of: Vec::new(),
            yields: Vec::new(),
            xp: 1,
        }
    }

    fn with_tools(mut self, tools: &[ToolId]) -> Self {
        self.tools.extend_from_slice(tools);
        self
    }

    fn with_supply(mut self, item: SupplyId, amount: u16) -> Self {
        self.supplies.push(SupplyCost { item, amount });
        self
    }

    fn with_any_of(mut self, options: &[SupplyCost]) -> Self {
        self.any_of.extend_from_slice(options);
        self
    }

    /// The choice clause as a player reads it: "1 sound plank or 1 fallen log".
    fn choice_text(&self) -> Option<String> {
        (!self.any_of.is_empty()).then(|| {
            self.any_of
                .iter()
                .map(|cost| format!("{} {}", cost.amount, cost.item.label_for(cost.amount)))
                .collect::<Vec<_>>()
                .join(" or ")
        })
    }

    fn with_yield(mut self, item: SupplyId, amount: u16) -> Self {
        self.yields.push(SupplyCost { item, amount });
        self
    }

    const fn without_experience(mut self) -> Self {
        self.xp = 0;
        self
    }

    pub fn requirements_text(&self) -> String {
        let mut parts = Vec::new();
        if self.level > 0 {
            parts.push(format!("{} {}", self.skill.label(), self.level));
        }
        parts.extend(self.tools.iter().map(|tool| tool.label().to_owned()));
        parts.extend(
            self.supplies
                .iter()
                .map(|cost| format!("{} {}", cost.amount, cost.item.label_for(cost.amount))),
        );
        parts.extend(self.choice_text());
        if parts.is_empty() {
            "no prerequisites".to_owned()
        } else {
            parts.join(" · ")
        }
    }

    pub fn yields_text(&self) -> String {
        self.yields
            .iter()
            .map(|gain| format!("{} {}", gain.amount, gain.item.label_for(gain.amount)))
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// The sawbuck turns the valley's fallen wood into the planks every
    /// carpentry and roofing repair asks for. Milling is craft, not restoration,
    /// so it teaches nothing on its own.
    pub fn for_milling() -> Self {
        Self::new(TaskAction::Mill, SkillId::Carpentry, 0)
            .with_tools(&[ToolId::Hatchet])
            .with_supply(SupplyId::Log, 1)
            .with_yield(SupplyId::Plank, 2)
            .without_experience()
    }

    /// Stone comes out of the valley's own outcrops, which is why the shed's
    /// broken pickaxe has to be made serviceable before any masonry can begin.
    pub fn for_quarrying() -> Self {
        Self::new(TaskAction::Quarry, SkillId::Masonry, 0)
            .with_tools(&[ToolId::Pickaxe])
            .with_yield(SupplyId::Stone, 3)
            .without_experience()
    }

    /// A tool comes back into service for a hammer, a practised hand, and wood
    /// for the haft — a fallen log or a sound plank, whichever is nearer.
    ///
    /// A broken hammer is the one exception on tools, since it cannot be the
    /// thing that mends itself. It still wants the skill and the wood.
    pub fn for_tool_repair(tool: ToolId) -> Self {
        let task =
            Self::new(TaskAction::Repair, SkillId::Upkeep, TOOL_REPAIR_LEVEL).with_any_of(&[
                SupplyCost {
                    item: SupplyId::Plank,
                    amount: 1,
                },
                SupplyCost {
                    item: SupplyId::Log,
                    amount: 1,
                },
            ]);
        if tool == ToolId::Hammer {
            task
        } else {
            task.with_tools(&[ToolId::Hammer])
        }
    }

    pub fn for_tree_chopping() -> Self {
        Self::new(TaskAction::Clear, SkillId::Upkeep, 0)
            .with_tools(&[ToolId::Hatchet])
            .with_yield(SupplyId::Log, 2)
            .with_yield(SupplyId::Kindling, 2)
    }

    /// The motel's parking bays are the only ground in the valley with soil from
    /// before the ash under them. Levering the slabs up is a one-time job per
    /// bed, it wants the pick, and the broken concrete is worth keeping.
    ///
    /// The bays carry their own copy of this, authored on the `parking-bay`
    /// repair pairs so the lot stays editable; a test holds the two in step.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn for_breaking_ground() -> Self {
        Self::new(TaskAction::Break, SkillId::Cultivation, 0)
            .with_tools(&[ToolId::Pickaxe])
            .with_yield(SupplyId::Stone, 2)
    }

    pub fn for_tilling() -> Self {
        Self::new(TaskAction::Till, SkillId::Cultivation, 0).with_tools(&[ToolId::Hoe])
    }

    pub fn for_sowing() -> Self {
        Self::new(TaskAction::Sow, SkillId::Cultivation, 0).with_supply(SupplyId::Seed, 1)
    }

    pub fn for_watering() -> Self {
        Self::new(TaskAction::Water, SkillId::Cultivation, 0)
            .with_tools(&[ToolId::WateringCan])
            .with_supply(SupplyId::Water, 1)
    }

    /// The only step that gives back more than it took. A sown seed returns two,
    /// so a garden that survives one season can be a wider one the next.
    pub fn for_harvest() -> Self {
        Self::new(TaskAction::Harvest, SkillId::Cultivation, 0)
            .with_yield(SupplyId::Ration, 3)
            .with_yield(SupplyId::Seed, 2)
    }

    /// Carrying water is fetching, not farming; it teaches the Scribe nothing.
    pub fn for_drawing_water() -> Self {
        Self::new(TaskAction::Draw, SkillId::Cultivation, 0)
            .with_tools(&[ToolId::WateringCan])
            .with_yield(SupplyId::Water, 3)
            .without_experience()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SkillProgress {
    xp: u16,
}

impl SkillProgress {
    pub fn level(&self) -> u8 {
        u8::try_from((self.xp / XP_PER_LEVEL).min(u16::from(MAX_SKILL_LEVEL)))
            .expect("capped skill level fits in u8")
    }

    pub const fn xp_into_level(&self) -> u16 {
        self.xp % XP_PER_LEVEL
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Resource, Serialize)]
pub struct Progression {
    #[serde(default)]
    supplies: BTreeMap<SupplyId, u16>,
    #[serde(default)]
    tools: BTreeSet<ToolId>,
    #[serde(default)]
    tool_instances: BTreeMap<String, PortableToolRecord>,
    #[serde(default)]
    equipped_tool: Option<String>,
    #[serde(default)]
    skills: BTreeMap<SkillId, SkillProgress>,
    #[serde(default)]
    collected_pickups: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskOutcome {
    pub old_level: u8,
    pub new_level: u8,
}

impl Progression {
    pub fn supply(&self, item: SupplyId) -> u16 {
        self.supplies.get(&item).copied().unwrap_or_default()
    }

    pub fn add_supply(&mut self, item: SupplyId, amount: u16) {
        *self.supplies.entry(item).or_default() += amount;
    }

    pub fn spend_supply(&mut self, item: SupplyId, amount: u16) -> bool {
        if self.supply(item) < amount {
            return false;
        }
        *self.supplies.entry(item).or_default() -= amount;
        true
    }

    pub fn has_tool(&self, tool: ToolId) -> bool {
        self.tools.contains(&tool)
            || self.tool_instances.values().any(|record| {
                record.tool == tool
                    && record.condition == ToolCondition::Serviceable
                    && record.location == ToolLocation::Carried
            })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn add_tool(&mut self, tool: ToolId) -> bool {
        self.tools.insert(tool)
    }

    pub fn register_tool_instance(&mut self, id: &str, tool: ToolId, condition: ToolCondition) {
        self.tool_instances
            .entry(id.to_owned())
            .or_insert(PortableToolRecord {
                tool,
                condition,
                location: ToolLocation::Home,
            });
    }

    pub fn tool_record(&self, id: &str) -> Option<&PortableToolRecord> {
        self.tool_instances.get(id)
    }

    pub fn carried_tool_count(&self) -> usize {
        self.tool_instances
            .values()
            .filter(|record| record.location == ToolLocation::Carried)
            .count()
            + self.tools.len()
    }

    pub fn pick_up_tool(&mut self, id: &str) -> Result<ToolId, String> {
        if self.carried_tool_count() >= MAX_CARRIED_TOOLS {
            return Err(format!(
                "You can carry only {MAX_CARRIED_TOOLS} tools. Return or drop one first."
            ));
        }
        let record = self
            .tool_instances
            .get_mut(id)
            .ok_or_else(|| format!("Unknown portable tool: {id}"))?;
        record.location = ToolLocation::Carried;
        self.equipped_tool = Some(id.to_owned());
        Ok(record.tool)
    }

    /// The tool `R` works on when nothing underfoot wants the key: whatever is
    /// in hand if it needs mending, otherwise the first broken thing carried.
    /// A tool goes where the Scribe goes, so mending one is not tied to
    /// standing in the shed where it was found.
    pub fn carried_broken_tool(&self) -> Option<(String, ToolId)> {
        let needs_mending = |record: &&PortableToolRecord| {
            record.location == ToolLocation::Carried && record.condition == ToolCondition::Broken
        };
        self.equipped_tool
            .as_ref()
            .and_then(|id| {
                self.tool_instances
                    .get(id)
                    .filter(needs_mending)
                    .map(|record| (id.clone(), record.tool))
            })
            .or_else(|| {
                self.tool_instances
                    .iter()
                    .find(|(_, record)| needs_mending(record))
                    .map(|(id, record)| (id.clone(), record.tool))
            })
    }

    pub fn set_tool_condition(&mut self, id: &str, condition: ToolCondition) -> bool {
        let Some(record) = self.tool_instances.get_mut(id) else {
            return false;
        };
        record.condition = condition;
        true
    }

    fn equipped_or_first_carried_id(&self) -> Option<String> {
        self.equipped_tool
            .as_ref()
            .filter(|id| {
                self.tool_instances
                    .get(*id)
                    .is_some_and(|record| record.location == ToolLocation::Carried)
            })
            .cloned()
            .or_else(|| {
                self.tool_instances
                    .iter()
                    .find(|(_, record)| record.location == ToolLocation::Carried)
                    .map(|(id, _)| id.clone())
            })
    }

    pub fn equipped_tool_id(&self) -> Option<String> {
        self.equipped_or_first_carried_id()
    }

    pub fn drop_equipped_tool(&mut self, scene_id: &str, position: (i32, i32)) -> Option<ToolId> {
        let id = self.equipped_or_first_carried_id()?;
        let record = self.tool_instances.get_mut(&id)?;
        record.location = ToolLocation::Dropped {
            scene_id: scene_id.to_owned(),
            x: position.0,
            y: position.1,
        };
        self.equipped_tool = None;
        Some(record.tool)
    }

    pub fn return_equipped_tool(&mut self) -> Option<ToolId> {
        let id = self.equipped_or_first_carried_id()?;
        let record = self.tool_instances.get_mut(&id)?;
        record.location = ToolLocation::Home;
        self.equipped_tool = None;
        Some(record.tool)
    }

    pub fn cycle_equipped_tool(&mut self) -> Option<ToolId> {
        let carried = self
            .tool_instances
            .iter()
            .filter(|(_, record)| record.location == ToolLocation::Carried)
            .map(|(id, record)| (id.clone(), record.tool))
            .collect::<Vec<_>>();
        let next_index = self
            .equipped_tool
            .as_ref()
            .and_then(|current| carried.iter().position(|(id, _)| id == current))
            .map_or(0, |index| (index + 1) % carried.len().max(1));
        let (id, tool) = carried.get(next_index)?.clone();
        self.equipped_tool = Some(id);
        Some(tool)
    }

    pub fn skill_level(&self, skill: SkillId) -> u8 {
        self.skills.get(&skill).map_or(0, SkillProgress::level)
    }

    pub fn skill_unlocked(&self, skill: SkillId) -> bool {
        match skill {
            SkillId::Upkeep => true,
            // Coaxing anything out of ash is patience learned on easier work first.
            SkillId::Carpentry | SkillId::Masonry | SkillId::Cultivation => {
                self.skill_level(SkillId::Upkeep) >= 1
            }
            SkillId::Roofing => self.skill_level(SkillId::Carpentry) >= 1,
        }
    }

    /// Everything standing between the Scribe and one job, phrased for a
    /// player. `attempt` reports only the first thing it hits, because it stops
    /// there; a prompt has room to name them all, and an action that can refuse
    /// owes the player the whole reason.
    pub fn shortfalls(&self, task: &TaskSpec) -> Vec<String> {
        let mut missing = Vec::new();
        if !self.skill_unlocked(task.skill) {
            missing.push(format!("{} (still locked)", task.skill.label()));
        } else if self.skill_level(task.skill) < task.level {
            missing.push(format!("{} {}", task.skill.label(), task.level));
        }
        missing.extend(
            task.tools
                .iter()
                .filter(|tool| !self.has_tool(**tool))
                .map(|tool| format!("a {}", tool.label())),
        );
        missing.extend(
            task.supplies
                .iter()
                .filter(|cost| self.supply(cost.item) < cost.amount)
                .map(|cost| {
                    format!(
                        "{} {} (you have {})",
                        cost.amount,
                        cost.item.label_for(cost.amount),
                        self.supply(cost.item)
                    )
                }),
        );
        if self.affordable_choice(task).is_none() {
            missing.extend(task.choice_text());
        }
        missing
    }

    /// Which option of a task's choice clause the Scribe can actually pay, if
    /// any. The first affordable one wins, so authoring order is preference
    /// order — a plank before a whole log, since the log is worth more milled.
    fn affordable_choice(&self, task: &TaskSpec) -> Option<SupplyCost> {
        task.any_of
            .iter()
            .copied()
            .find(|cost| self.supply(cost.item) >= cost.amount)
    }

    pub fn attempt(&mut self, task: &TaskSpec) -> Result<TaskOutcome, String> {
        if !self.skill_unlocked(task.skill) {
            let unlock = match task.skill {
                SkillId::Carpentry | SkillId::Masonry | SkillId::Cultivation => {
                    "Reach Upkeep 1 first."
                }
                SkillId::Roofing => "Reach Carpentry 1 first.",
                SkillId::Upkeep => "",
            };
            return Err(format!("{} is not unlocked. {unlock}", task.skill.label()));
        }
        let level = self.skill_level(task.skill);
        if level < task.level {
            return Err(format!(
                "Needs {} {}; current level is {}.",
                task.skill.label(),
                task.level,
                level
            ));
        }
        if let Some(tool) = task.tools.iter().find(|tool| !self.has_tool(**tool)) {
            return Err(format!("Needs a {}.", tool.label()));
        }
        if let Some(cost) = task
            .supplies
            .iter()
            .find(|cost| self.supply(cost.item) < cost.amount)
        {
            return Err(format!(
                "Needs {} {}; you have {}.",
                cost.amount,
                cost.item.label_for(cost.amount),
                self.supply(cost.item)
            ));
        }
        let choice = self.affordable_choice(task);
        if choice.is_none() {
            if let Some(wanted) = task.choice_text() {
                // No count to report: had the Scribe any of it, this would
                // have gone through instead of coming back here.
                return Err(format!("Needs {wanted}."));
            }
        }
        for cost in task.supplies.iter().chain(choice.iter()) {
            *self.supplies.entry(cost.item).or_default() -= cost.amount;
        }
        for gain in &task.yields {
            *self.supplies.entry(gain.item).or_default() += gain.amount;
        }
        let old_level = level;
        self.skills.entry(task.skill).or_default().xp += task.xp;
        Ok(TaskOutcome {
            old_level,
            new_level: self.skill_level(task.skill),
        })
    }

    pub fn pickup_collected(&self, id: &str) -> bool {
        self.collected_pickups.contains(id)
    }

    pub fn collect_pickup(&mut self, id: &str) {
        self.collected_pickups.insert(id.to_owned());
    }

    pub fn tools_summary(&self) -> String {
        let mut carried = self
            .tool_instances
            .values()
            .filter(|record| record.location == ToolLocation::Carried)
            .map(|record| {
                if record.condition == ToolCondition::Broken {
                    format!("{} ({})", record.tool.label(), record.condition.label())
                } else {
                    record.tool.label().to_owned()
                }
            })
            .collect::<Vec<_>>();
        carried.extend(self.tools.iter().map(|tool| tool.label().to_owned()));
        if carried.is_empty() {
            return "none yet".to_owned();
        }
        format!(
            "carried {}/{}: {}",
            carried.len(),
            MAX_CARRIED_TOOLS,
            carried.join(", ")
        )
    }

    pub fn supplies_summary(&self) -> String {
        SupplyId::ALL
            .into_iter()
            .filter_map(|item| {
                let amount = self.supply(item);
                (amount > 0).then(|| format!("{amount} {}", item.label()))
            })
            .collect::<Vec<_>>()
            .join(" · ")
    }

    pub fn skill_tree_summary(&self) -> String {
        SkillId::ALL
            .into_iter()
            .map(|skill| {
                if self.skill_unlocked(skill) {
                    let progress = self.skills.get(&skill).cloned().unwrap_or_default();
                    let level = progress.level();
                    if level >= MAX_SKILL_LEVEL {
                        format!("{} {} MAX", skill.label(), level)
                    } else {
                        format!(
                            "{} {} ({}/{})",
                            skill.label(),
                            level,
                            progress.xp_into_level(),
                            XP_PER_LEVEL
                        )
                    }
                } else {
                    format!("{} — locked", skill.label())
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleaning_unlocks_the_first_specialties() {
        let mut progression = Progression::default();
        let clean = TaskSpec::for_kind("debris");
        for _ in 0..3 {
            progression
                .attempt(&clean)
                .expect("cleaning has no requirements");
        }
        assert_eq!(progression.skill_level(SkillId::Upkeep), 1);
        assert!(progression.skill_unlocked(SkillId::Carpentry));
        assert!(progression.skill_unlocked(SkillId::Masonry));
        assert!(!progression.skill_unlocked(SkillId::Roofing));
    }

    #[test]
    fn a_task_reports_and_consumes_requirements_in_order() {
        let mut progression = Progression::default();
        for _ in 0..3 {
            progression
                .attempt(&TaskSpec::for_kind("debris"))
                .expect("cleaning");
        }
        let floor = TaskSpec::for_kind("floor");
        assert_eq!(
            progression.attempt(&floor),
            Err("Needs a hammer.".to_owned())
        );
        progression.add_tool(ToolId::Hammer);
        assert_eq!(
            progression.attempt(&floor),
            Err("Needs 1 sound plank; you have 0.".to_owned())
        );
        progression.add_supply(SupplyId::Plank, 1);
        progression.attempt(&floor).expect("requirements met");
        assert_eq!(progression.supply(SupplyId::Plank), 0);
    }

    #[test]
    fn cleared_debris_gives_back_the_nails_it_was_built_with() {
        let mut progression = Progression::default();
        progression
            .attempt(&TaskSpec::for_kind("debris"))
            .expect("cleaning has no requirements");

        assert_eq!(progression.supply(SupplyId::Nails), 1);
    }

    #[test]
    fn the_sawbuck_turns_valley_logs_into_carpentry_planks() {
        let mut progression = Progression::default();
        let milling = TaskSpec::for_milling();
        progression.add_tool(ToolId::Hatchet);
        progression.add_supply(SupplyId::Log, 1);

        // Milling is carpentry work, so it waits on the same Upkeep 1 unlock.
        assert_eq!(
            progression.attempt(&milling),
            Err("Carpentry is not unlocked. Reach Upkeep 1 first.".to_owned())
        );
        for _ in 0..3 {
            progression
                .attempt(&TaskSpec::for_kind("debris"))
                .expect("cleaning");
        }
        progression.attempt(&milling).expect("a log and a hatchet");

        assert_eq!(progression.supply(SupplyId::Log), 0);
        assert_eq!(progression.supply(SupplyId::Plank), 2);
        // Craft, not restoration: the plank is the reward, not the lesson.
        assert_eq!(progression.skill_level(SkillId::Carpentry), 0);
    }

    #[test]
    fn quarrying_stone_waits_on_a_serviceable_pickaxe() {
        let mut progression = Progression::default();
        progression.register_tool_instance("pickaxe-01", ToolId::Pickaxe, ToolCondition::Broken);
        progression.pick_up_tool("pickaxe-01").expect("carried");
        progression.add_tool(ToolId::Hammer);
        progression.add_tool(ToolId::Hatchet);
        for _ in 0..u16::from(TOOL_REPAIR_LEVEL) * XP_PER_LEVEL {
            progression
                .attempt(&TaskSpec::for_kind("debris"))
                .expect("cleaning");
        }
        let quarrying = TaskSpec::for_quarrying();

        assert_eq!(
            progression.attempt(&quarrying),
            Err("Needs a pickaxe.".to_owned())
        );
        // The haft wants wood before the pick will hold an edge, and the valley
        // hands that over one felled tree at a time.
        assert_eq!(
            progression.attempt(&TaskSpec::for_tool_repair(ToolId::Pickaxe)),
            Err("Needs 1 sound plank or 1 fallen log.".to_owned())
        );
        progression
            .attempt(&TaskSpec::for_tree_chopping())
            .expect("a hatchet and a standing tree");
        progression
            .attempt(&TaskSpec::for_tool_repair(ToolId::Pickaxe))
            .expect("Upkeep 2, a hammer, and wood for the haft");
        progression.set_tool_condition("pickaxe-01", ToolCondition::Serviceable);
        progression.attempt(&quarrying).expect("a working pickaxe");

        assert_eq!(progression.supply(SupplyId::Stone), 3);
    }

    #[test]
    fn masonry_becomes_reachable_once_stone_can_be_quarried() {
        let mut progression = Progression::default();
        progression.add_tool(ToolId::Pickaxe);
        progression.add_tool(ToolId::Shovel);
        for _ in 0..3 {
            progression
                .attempt(&TaskSpec::for_kind("debris"))
                .expect("cleaning");
        }
        let wall = TaskSpec::for_kind("wall");
        assert_eq!(
            progression.attempt(&wall),
            Err("Needs 1 stone; you have 0.".to_owned())
        );

        for _ in 0..3 {
            progression
                .attempt(&TaskSpec::for_quarrying())
                .expect("an outcrop");
        }
        for _ in 0..3 {
            progression.attempt(&wall).expect("stone in hand");
        }

        assert_eq!(progression.skill_level(SkillId::Masonry), 1);
    }

    /// The whole chain the restoration depends on, walked end to end: clean to
    /// unlock, salvage the nails, chop and mill for planks, repair the pickaxe,
    /// quarry the stone, and put every specialty to work.
    #[test]
    fn one_valley_loop_carries_every_skill_to_its_ceiling() {
        let mut progression = Progression::default();
        // The shed's pickaxe is the one tool that starts broken; the rest stand
        // in for trips the Scribe makes inside the three-tool carry limit.
        progression.register_tool_instance("pickaxe-01", ToolId::Pickaxe, ToolCondition::Broken);
        progression.pick_up_tool("pickaxe-01").expect("carried");
        progression.add_tool(ToolId::Hammer);
        progression.add_tool(ToolId::Hatchet);
        progression.add_tool(ToolId::Shovel);
        progression.add_tool(ToolId::Ladder);

        let clean = TaskSpec::for_kind("debris");
        let floor = TaskSpec::for_kind("floor");
        let wall = TaskSpec::for_kind("wall");
        let roof = TaskSpec::for_kind("roof");
        for _ in 0..9 {
            progression.attempt(&clean).expect("debris needs nothing");
        }
        assert_eq!(progression.skill_level(SkillId::Upkeep), MAX_SKILL_LEVEL);

        // Wood comes before the pick does: mending a haft needs something to
        // cut one from, so the axe work leads and the quarry waits on it.
        for _ in 0..9 {
            progression
                .attempt(&TaskSpec::for_tree_chopping())
                .expect("a hatchet and a standing tree");
        }
        progression
            .attempt(&TaskSpec::for_tool_repair(ToolId::Pickaxe))
            .expect("Upkeep 2, a hammer, and wood for the haft");
        progression.set_tool_condition("pickaxe-01", ToolCondition::Serviceable);

        for _ in 0..17 {
            progression
                .attempt(&TaskSpec::for_milling())
                .expect("a log on the sawbuck");
        }
        for _ in 0..3 {
            progression
                .attempt(&TaskSpec::for_quarrying())
                .expect("a working pickaxe");
        }

        for _ in 0..9 {
            progression.attempt(&floor).expect("planks for carpentry");
            progression.attempt(&wall).expect("stone for masonry");
        }
        // Roofing opens only behind Carpentry, which the floorboards just paid for.
        for _ in 0..9 {
            progression.attempt(&roof).expect("planks and a ladder");
        }

        // The garden is the one chain that feeds itself: one scavenged seed and
        // one trip to the river open a season that hands back more than it took.
        progression.add_tool(ToolId::Hoe);
        progression.add_tool(ToolId::WateringCan);
        progression.add_supply(SupplyId::Seed, 1);
        // The pick was made serviceable back at the top of this walk.
        progression
            .attempt(&TaskSpec::for_drawing_water())
            .expect("a can at the river");
        for _ in 0..3 {
            for step in [
                TaskSpec::for_breaking_ground(),
                TaskSpec::for_tilling(),
                TaskSpec::for_sowing(),
                TaskSpec::for_watering(),
                TaskSpec::for_harvest(),
            ] {
                progression
                    .attempt(&step)
                    .unwrap_or_else(|reason| panic!("garden step should be payable: {reason}"));
            }
        }
        assert!(progression.supply(SupplyId::Seed) > 1, "the garden widens");

        for skill in SkillId::ALL {
            assert_eq!(
                progression.skill_level(skill),
                MAX_SKILL_LEVEL,
                "{} stalled below its ceiling",
                skill.label()
            );
        }
    }

    #[test]
    fn portable_tools_have_condition_location_and_a_small_carry_limit() {
        let mut progression = Progression::default();
        for (id, tool, condition) in [
            ("hammer-01", ToolId::Hammer, ToolCondition::Serviceable),
            ("axe-01", ToolId::Hatchet, ToolCondition::Serviceable),
            ("shovel-01", ToolId::Shovel, ToolCondition::Serviceable),
            ("pickaxe-01", ToolId::Pickaxe, ToolCondition::Broken),
        ] {
            progression.register_tool_instance(id, tool, condition);
        }
        progression.pick_up_tool("hammer-01").expect("first tool");
        progression.pick_up_tool("axe-01").expect("second tool");
        progression.pick_up_tool("shovel-01").expect("third tool");
        assert!(progression.pick_up_tool("pickaxe-01").is_err());
        assert!(progression.has_tool(ToolId::Hammer));
        assert!(!progression.has_tool(ToolId::Pickaxe));

        progression.drop_equipped_tool("exterior", (12, -8));
        assert_eq!(
            progression
                .tool_record("shovel-01")
                .map(|record| &record.location),
            Some(&ToolLocation::Dropped {
                scene_id: "exterior".to_owned(),
                x: 12,
                y: -8,
            })
        );
        progression
            .pick_up_tool("pickaxe-01")
            .expect("a freed carry slot");
        assert!(!progression.has_tool(ToolId::Pickaxe));
    }

    /// A hand and a hammer are not enough; the haft wants wood. Either kind
    /// pays, and only one of them is spent — a tool repair should not quietly
    /// eat both the plank and the log when it needed one of them.
    #[test]
    fn mending_a_tool_wants_wood_and_takes_whichever_kind_is_nearer() {
        let repair = TaskSpec::for_tool_repair(ToolId::Pickaxe);
        let ready = || {
            let mut progression = Progression::default();
            progression.add_tool(ToolId::Hammer);
            for _ in 0..u16::from(TOOL_REPAIR_LEVEL) * XP_PER_LEVEL {
                progression
                    .attempt(&TaskSpec::for_kind("debris"))
                    .expect("cleaning asks for nothing");
            }
            progression
        };

        let mut empty_handed = ready();
        assert_eq!(
            empty_handed.attempt(&repair),
            Err("Needs 1 sound plank or 1 fallen log.".to_owned())
        );
        assert!(empty_handed
            .shortfalls(&repair)
            .contains(&"1 sound plank or 1 fallen log".to_owned()));

        let mut with_a_log = ready();
        with_a_log.add_supply(SupplyId::Log, 1);
        with_a_log.attempt(&repair).expect("a log is wood enough");
        assert_eq!(with_a_log.supply(SupplyId::Log), 0);

        // Both on hand: the plank goes, because a whole log is worth more
        // milled than whittled down for one handle.
        let mut with_both = ready();
        with_both.add_supply(SupplyId::Log, 1);
        with_both.add_supply(SupplyId::Plank, 1);
        with_both.attempt(&repair).expect("either will do");
        assert_eq!(with_both.supply(SupplyId::Plank), 0);
        assert_eq!(with_both.supply(SupplyId::Log), 1);
    }

    /// R with nothing underfoot works on what the Scribe is carrying, so this
    /// has to name the right thing: the tool in hand when that one is broken,
    /// and never a tool that is sound or still lying where it was found.
    #[test]
    fn the_carried_tool_r_reaches_for_is_the_broken_one_in_hand() {
        let mut progression = Progression::default();
        progression.register_tool_instance("hoe-01", ToolId::Hoe, ToolCondition::Broken);
        progression.register_tool_instance("pickaxe-01", ToolId::Pickaxe, ToolCondition::Broken);
        progression.register_tool_instance("hammer-01", ToolId::Hammer, ToolCondition::Serviceable);

        // Still on the shed floor: nothing to work on.
        assert_eq!(progression.carried_broken_tool(), None);

        progression.pick_up_tool("hammer-01").expect("carried");
        assert_eq!(progression.carried_broken_tool(), None);

        progression.pick_up_tool("pickaxe-01").expect("carried");
        assert_eq!(
            progression.carried_broken_tool(),
            Some(("pickaxe-01".to_owned(), ToolId::Pickaxe))
        );

        // Two broken tools in the pack: the equipped one is the one meant.
        progression.pick_up_tool("hoe-01").expect("carried");
        assert_eq!(
            progression.carried_broken_tool(),
            Some(("hoe-01".to_owned(), ToolId::Hoe))
        );

        progression.set_tool_condition("hoe-01", ToolCondition::Serviceable);
        assert_eq!(
            progression.carried_broken_tool(),
            Some(("pickaxe-01".to_owned(), ToolId::Pickaxe))
        );
        progression.set_tool_condition("pickaxe-01", ToolCondition::Serviceable);
        assert_eq!(progression.carried_broken_tool(), None);
    }

    #[test]
    fn a_broken_tool_can_be_repaired_and_saved_by_stable_id() {
        let mut progression = Progression::default();
        progression.register_tool_instance("pickaxe-01", ToolId::Pickaxe, ToolCondition::Broken);
        progression
            .pick_up_tool("pickaxe-01")
            .expect("portable tool");
        progression.add_tool(ToolId::Hammer);
        for _ in 0..u16::from(TOOL_REPAIR_LEVEL) * XP_PER_LEVEL {
            progression
                .attempt(&TaskSpec::for_kind("debris"))
                .expect("basic upkeep");
        }
        progression.add_supply(SupplyId::Plank, 1);
        progression
            .attempt(&TaskSpec::for_tool_repair(ToolId::Pickaxe))
            .expect("repair requirements met");
        progression.set_tool_condition("pickaxe-01", ToolCondition::Serviceable);

        let serialized = serde_json::to_string(&progression).expect("serialize progression");
        let restored: Progression = serde_json::from_str(&serialized).expect("restore progression");
        assert!(restored.has_tool(ToolId::Pickaxe));
        assert_eq!(
            restored
                .tool_record("pickaxe-01")
                .map(|record| record.condition),
            Some(ToolCondition::Serviceable)
        );
    }
}
