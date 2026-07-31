//! Small, data-driven restoration progression for the motel repair loop.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

const XP_PER_LEVEL: u16 = 3;
const MAX_SKILL_LEVEL: u8 = 3;
pub const MAX_CARRIED_TOOLS: usize = 3;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillId {
    Upkeep,
    Carpentry,
    Masonry,
    Roofing,
}

impl SkillId {
    pub const ALL: [Self; 4] = [Self::Upkeep, Self::Carpentry, Self::Masonry, Self::Roofing];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Upkeep => "Upkeep",
            Self::Carpentry => "Carpentry",
            Self::Masonry => "Masonry",
            Self::Roofing => "Roofing",
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
}

impl SupplyId {
    pub const ALL: [Self; 6] = [
        Self::Kindling,
        Self::Log,
        Self::Plank,
        Self::Nails,
        Self::Stone,
        Self::Cloth,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Kindling => "kindling",
            Self::Log => "fallen logs",
            Self::Plank => "sound planks",
            Self::Nails => "nails",
            Self::Stone => "stone",
            Self::Cloth => "cloth",
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
}

impl TaskAction {
    pub const fn infinitive(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Repair => "repair",
            Self::Clear => "clear",
            Self::Restore => "restore",
            Self::Light => "light",
        }
    }

    pub const fn past_tense(self) -> &'static str {
        match self {
            Self::Clean => "cleaned",
            Self::Repair => "repaired",
            Self::Clear => "cleared",
            Self::Restore => "restored",
            Self::Light => "lit",
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
            "debris" => Self::new(TaskAction::Clean, SkillId::Upkeep, 0),
            "floor" => Self::new(TaskAction::Repair, SkillId::Carpentry, 0)
                .with_tools(&[ToolId::Hammer])
                .with_supply(SupplyId::Plank, 1),
            "furniture" | "furnitute" | "bed" | "nightstand" => {
                Self::new(TaskAction::Repair, SkillId::Carpentry, 1)
                    .with_tools(&[ToolId::Hammer])
                    .with_supply(SupplyId::Plank, 1)
            }
            "wall" | "fireplace" => Self::new(TaskAction::Repair, SkillId::Masonry, 0)
                .with_tools(&[ToolId::Trowel])
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

    pub fn requirements_text(&self) -> String {
        let mut parts = Vec::new();
        if self.level > 0 {
            parts.push(format!("{} {}", self.skill.label(), self.level));
        }
        parts.extend(self.tools.iter().map(|tool| tool.label().to_owned()));
        parts.extend(
            self.supplies
                .iter()
                .map(|cost| format!("{} {}", cost.amount, cost.item.label())),
        );
        if parts.is_empty() {
            "no prerequisites".to_owned()
        } else {
            parts.join(" · ")
        }
    }

    pub fn for_tool_repair(tool: ToolId) -> Self {
        let task = Self::new(TaskAction::Repair, SkillId::Upkeep, 1);
        if tool == ToolId::Hammer {
            task
        } else {
            task.with_tools(&[ToolId::Hammer])
        }
    }

    pub fn for_tree_chopping() -> Self {
        Self::new(TaskAction::Clear, SkillId::Upkeep, 0).with_tools(&[ToolId::Hatchet])
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
            SkillId::Carpentry | SkillId::Masonry => self.skill_level(SkillId::Upkeep) >= 1,
            SkillId::Roofing => self.skill_level(SkillId::Carpentry) >= 1,
        }
    }

    pub fn attempt(&mut self, task: &TaskSpec) -> Result<TaskOutcome, String> {
        if !self.skill_unlocked(task.skill) {
            let unlock = match task.skill {
                SkillId::Carpentry | SkillId::Masonry => "Reach Upkeep 1 first.",
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
                cost.item.label(),
                self.supply(cost.item)
            ));
        }
        for cost in &task.supplies {
            *self.supplies.entry(cost.item).or_default() -= cost.amount;
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
                    format!("{} (broken)", record.tool.label())
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
            Err("Needs 1 sound planks; you have 0.".to_owned())
        );
        progression.add_supply(SupplyId::Plank, 1);
        progression.attempt(&floor).expect("requirements met");
        assert_eq!(progression.supply(SupplyId::Plank), 0);
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

    #[test]
    fn a_broken_tool_can_be_repaired_and_saved_by_stable_id() {
        let mut progression = Progression::default();
        progression.register_tool_instance("pickaxe-01", ToolId::Pickaxe, ToolCondition::Broken);
        progression
            .pick_up_tool("pickaxe-01")
            .expect("portable tool");
        progression.add_tool(ToolId::Hammer);
        for _ in 0..3 {
            progression
                .attempt(&TaskSpec::for_kind("debris"))
                .expect("basic upkeep");
        }
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
