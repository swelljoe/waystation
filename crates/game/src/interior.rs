//! Authored interior and building metadata, positioning, and runtime art.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use serde::Deserialize;

use crate::progression::TaskSpec;

pub const INTERIOR_ORIGIN: Vec2 = Vec2::new(8_192.0, 0.0);
pub const MOTEL_EXTERIOR_ORIGIN: Vec2 = Vec2::new(0.0, 100.0);

#[derive(Component)]
pub struct InteriorSceneEntity;

const MOTEL_OFFICE_JSON: &str = include_str!("../../../content/interiors/motel-office.json");
const MOTEL_ROOM_01_JSON: &str = include_str!("../../../content/interiors/motel-room-01.json");
const MOTEL_ROOM_02_JSON: &str = include_str!("../../../content/interiors/motel-room-02.json");
const MOTEL_ROOM_03_JSON: &str = include_str!("../../../content/interiors/motel-room-03.json");
const MOTEL_ROOM_04_JSON: &str = include_str!("../../../content/interiors/motel-room-04.json");
const MOTEL_ROOM_05_JSON: &str = include_str!("../../../content/interiors/motel-room-05.json");
const MOTEL_ROOM_06_JSON: &str = include_str!("../../../content/interiors/motel-room-06.json");
const MOTEL_EXTERIOR_JSON: &str = include_str!("../../../content/buildings/motel-exterior.json");
const REPAIR_PAIRS_JSON: &str = include_str!("../../../content/repair-pairs.json");

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum InteriorId {
    Office,
    Room01,
    Room02,
    Room03,
    Room04,
    Room05,
    Room06,
}

impl InteriorId {
    pub const ALL: [Self; 7] = [
        Self::Office,
        Self::Room01,
        Self::Room02,
        Self::Room03,
        Self::Room04,
        Self::Room05,
        Self::Room06,
    ];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Office => "motel-office",
            Self::Room01 => "motel-room-01",
            Self::Room02 => "motel-room-02",
            Self::Room03 => "motel-room-03",
            Self::Room04 => "motel-room-04",
            Self::Room05 => "motel-room-05",
            Self::Room06 => "motel-room-06",
        }
    }

    #[must_use]
    pub const fn door_label(self) -> &'static str {
        match self {
            Self::Office => "office",
            Self::Room01 => "room 1",
            Self::Room02 => "room 2",
            Self::Room03 => "room 3",
            Self::Room04 => "room 4",
            Self::Room05 => "room 5",
            Self::Room06 => "room 6",
        }
    }

    const fn json(self) -> &'static str {
        match self {
            Self::Office => MOTEL_OFFICE_JSON,
            Self::Room01 => MOTEL_ROOM_01_JSON,
            Self::Room02 => MOTEL_ROOM_02_JSON,
            Self::Room03 => MOTEL_ROOM_03_JSON,
            Self::Room04 => MOTEL_ROOM_04_JSON,
            Self::Room05 => MOTEL_ROOM_05_JSON,
            Self::Room06 => MOTEL_ROOM_06_JSON,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct Cell {
    pub x: u16,
    pub y: u16,
}

#[derive(Debug, Deserialize)]
struct GridDefinition {
    width: u16,
    height: u16,
    tile_size: u16,
}

#[derive(Debug, Deserialize)]
struct SceneDefinition {
    schema_version: u8,
    id: String,
    name: String,
    grid: GridDefinition,
    #[serde(default)]
    entry: Option<Cell>,
    #[serde(default)]
    exits: Vec<Cell>,
    #[serde(default)]
    collision: Vec<Cell>,
    #[serde(default)]
    templates: HashMap<String, ElementTemplateDefinition>,
    #[serde(default)]
    structures: Vec<MutableInstanceDefinition>,
    #[serde(default)]
    fixtures: Vec<MutableInstanceDefinition>,
}

#[derive(Debug, Deserialize)]
struct RepairPairLibrary {
    schema_version: u8,
    pairs: HashMap<String, ElementTemplateDefinition>,
}

#[derive(Debug, Deserialize)]
struct ElementTemplateDefinition {
    label: String,
    kind: String,
    layer: String,
    #[serde(default)]
    task: Option<TaskSpec>,
    states: HashMap<String, VisualDefinition>,
}

#[derive(Debug, Deserialize)]
struct MutableInstanceDefinition {
    id: String,
    template: String,
    #[serde(default)]
    layer: Option<String>,
    #[serde(default)]
    x: Option<i16>,
    #[serde(default)]
    y: Option<i16>,
    #[serde(default)]
    position: Option<PixelPositionDefinition>,
    initial_state: String,
    #[serde(default)]
    transform: InstanceTransformDefinition,
}

#[derive(Debug, Deserialize)]
struct PixelPositionDefinition {
    grid: u16,
    x: i32,
    y: i32,
}

impl MutableInstanceDefinition {
    #[allow(clippy::cast_precision_loss)]
    fn pixel_position(&self, tile_size: f32) -> Vec2 {
        self.position.as_ref().map_or_else(
            || {
                Vec2::new(
                    f32::from(self.x.expect("legacy instance needs x")) * tile_size,
                    f32::from(self.y.expect("legacy instance needs y")) * tile_size,
                )
            },
            |position| {
                Vec2::new(
                    position.x as f32 * f32::from(position.grid),
                    position.y as f32 * f32::from(position.grid),
                )
            },
        )
    }

    fn resolved_layer<'a>(&'a self, template_layer: &'a str) -> &'a str {
        self.layer.as_deref().unwrap_or(template_layer)
    }
}

#[derive(Debug, Default, Deserialize)]
struct InstanceTransformDefinition {
    #[serde(default)]
    flip_x: bool,
    #[serde(default)]
    flip_y: bool,
}

#[derive(Debug, Deserialize)]
struct VisualDefinition {
    #[serde(default = "visible_by_default")]
    visible: bool,
    source: Option<SourceDefinition>,
}

const fn visible_by_default() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct SourceDefinition {
    grid: u16,
    width: u16,
    height: u16,
}

#[derive(Clone, Debug)]
pub struct ElementVisual {
    pub image_path: Option<String>,
    pub size: Vec2,
    pub visible: bool,
}

#[derive(Clone, Debug)]
pub struct MutableElement {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub task: TaskSpec,
    pub layer: String,
    pub pixel_x: f32,
    pub pixel_y: f32,
    pub initial_state: String,
    pub flip_x: bool,
    pub flip_y: bool,
    pub states: HashMap<String, ElementVisual>,
}

#[derive(Debug)]
struct SceneMap {
    id: String,
    name: String,
    width: u16,
    height: u16,
    tile_size: f32,
    origin: Vec2,
    art_directory: &'static str,
    collision: HashSet<Cell>,
    mutable_elements: Vec<MutableElement>,
}

impl SceneMap {
    fn load(
        json: &str,
        expected_id: &str,
        origin: Vec2,
        art_directory: &'static str,
    ) -> (Self, Option<Cell>, Vec<Cell>) {
        let definition: SceneDefinition =
            serde_json::from_str(json).expect("authored scene must be valid JSON");
        let repair_pairs: RepairPairLibrary = serde_json::from_str(REPAIR_PAIRS_JSON)
            .expect("authored repair-pair library must be valid JSON");
        assert!(
            matches!(definition.schema_version, 1..=4),
            "unsupported authored-scene schema"
        );
        assert_eq!(repair_pairs.schema_version, 1);
        assert_eq!(definition.id, expected_id);

        let room_id = definition.id.clone();
        let templates = &definition.templates;
        let tile_size = f32::from(definition.grid.tile_size);
        let mutable_elements = definition
            .structures
            .into_iter()
            .chain(definition.fixtures)
            .map(|instance| {
                let template = if definition.schema_version >= 3 {
                    repair_pairs
                        .pairs
                        .get(&instance.template)
                        .or_else(|| templates.get(&instance.template))
                } else {
                    templates
                        .get(&instance.template)
                        .or_else(|| repair_pairs.pairs.get(&instance.template))
                }
                .unwrap_or_else(|| panic!("unknown repair pair: {}", instance.template));
                let pixel_position = instance.pixel_position(tile_size);
                let layer = instance.resolved_layer(&template.layer).to_owned();
                MutableElement {
                    id: instance.id,
                    label: template.label.clone(),
                    kind: template.kind.clone(),
                    task: template
                        .task
                        .clone()
                        .unwrap_or_else(|| TaskSpec::for_kind(&template.kind)),
                    layer,
                    pixel_x: pixel_position.x,
                    pixel_y: pixel_position.y,
                    initial_state: instance.initial_state,
                    flip_x: instance.transform.flip_x,
                    flip_y: instance.transform.flip_y,
                    states: template
                        .states
                        .iter()
                        .map(|(state_name, visual)| {
                            let size = visual.source.as_ref().map_or(Vec2::ZERO, |source| {
                                Vec2::new(
                                    f32::from(source.width) * f32::from(source.grid),
                                    f32::from(source.height) * f32::from(source.grid),
                                )
                            });
                            let image_path = visual.source.as_ref().map(|_| {
                                format!(
                                    "{art_directory}/{room_id}/{}--{state_name}.png",
                                    instance.template
                                )
                            });
                            (
                                state_name.clone(),
                                ElementVisual {
                                    image_path,
                                    size,
                                    visible: visual.visible,
                                },
                            )
                        })
                        .collect(),
                }
            })
            .collect();
        let entry = definition.entry;
        let exits = definition.exits;
        (
            Self {
                id: definition.id,
                name: definition.name,
                width: definition.grid.width,
                height: definition.grid.height,
                tile_size,
                origin,
                art_directory,
                collision: definition.collision.into_iter().collect(),
                mutable_elements,
            },
            entry,
            exits,
        )
    }

    fn world_size(&self) -> Vec2 {
        Vec2::new(
            f32::from(self.width) * self.tile_size,
            f32::from(self.height) * self.tile_size,
        )
    }

    fn cell_center(&self, cell: Cell) -> Vec2 {
        let size = self.world_size();
        Vec2::new(
            (f32::from(cell.x) + 0.5).mul_add(self.tile_size, self.origin.x - size.x / 2.0),
            (f32::from(cell.y) + 0.5).mul_add(-self.tile_size, self.origin.y + size.y / 2.0),
        )
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn cell_at(&self, position: Vec2) -> Option<Cell> {
        let size = self.world_size();
        let x = ((position.x - (self.origin.x - size.x / 2.0)) / self.tile_size).floor();
        let y = (((self.origin.y + size.y / 2.0) - position.y) / self.tile_size).floor();
        if x < 0.0 || y < 0.0 || x >= f32::from(self.width) || y >= f32::from(self.height) {
            return None;
        }
        Some(Cell {
            x: x as u16,
            y: y as u16,
        })
    }

    fn element_center(&self, element: &MutableElement, visual_size: Vec2) -> Vec2 {
        let room_size = self.world_size();
        let top_left = Vec2::new(
            element.pixel_x + self.origin.x - room_size.x / 2.0,
            -element.pixel_y + self.origin.y + room_size.y / 2.0,
        );
        top_left + Vec2::new(visual_size.x / 2.0, -visual_size.y / 2.0)
    }
}

#[derive(Resource, Debug)]
pub struct InteriorMap {
    scene: SceneMap,
    pub interior_id: InteriorId,
    pub entry: Cell,
    pub exits: Vec<Cell>,
}

impl InteriorMap {
    #[must_use]
    pub fn load(interior_id: InteriorId) -> Self {
        let (scene, entry, exits) = SceneMap::load(
            interior_id.json(),
            interior_id.id(),
            INTERIOR_ORIGIN,
            "interiors",
        );
        Self {
            scene,
            interior_id,
            entry: entry.expect("an interior must define an entry"),
            exits,
        }
    }

    pub fn id(&self) -> &str {
        &self.scene.id
    }

    pub fn name(&self) -> &str {
        &self.scene.name
    }

    pub fn world_size(&self) -> Vec2 {
        self.scene.world_size()
    }

    pub fn cell_center(&self, cell: Cell) -> Vec2 {
        self.scene.cell_center(cell)
    }

    pub fn is_walkable(&self, position: Vec2) -> bool {
        self.scene
            .cell_at(position)
            .is_some_and(|cell| !self.scene.collision.contains(&cell))
    }

    pub fn camera_position(&self, player: Vec2, camera_half_size: Vec2) -> Vec2 {
        let room_half_size = self.world_size() / 2.0;
        let travel = (room_half_size - camera_half_size).max(Vec2::ZERO);
        Vec2::new(
            player
                .x
                .clamp(INTERIOR_ORIGIN.x - travel.x, INTERIOR_ORIGIN.x + travel.x),
            player
                .y
                .clamp(INTERIOR_ORIGIN.y - travel.y, INTERIOR_ORIGIN.y + travel.y),
        )
    }

    pub fn mutable_elements(&self) -> &[MutableElement] {
        &self.scene.mutable_elements
    }

    pub fn mutable_element(&self, id: &str) -> Option<&MutableElement> {
        self.scene
            .mutable_elements
            .iter()
            .find(|element| element.id == id)
    }

    pub fn element_center(&self, element: &MutableElement, visual_size: Vec2) -> Vec2 {
        self.scene.element_center(element, visual_size)
    }
}

#[derive(Resource, Debug)]
pub struct MotelExteriorMap {
    scene: SceneMap,
}

impl MotelExteriorMap {
    #[must_use]
    pub fn load() -> Self {
        let (scene, _, _) = SceneMap::load(
            MOTEL_EXTERIOR_JSON,
            "motel-exterior",
            MOTEL_EXTERIOR_ORIGIN,
            "buildings",
        );
        Self { scene }
    }

    pub fn id(&self) -> &str {
        &self.scene.id
    }

    pub fn mutable_elements(&self) -> &[MutableElement] {
        &self.scene.mutable_elements
    }

    pub fn mutable_element(&self, id: &str) -> Option<&MutableElement> {
        self.scene
            .mutable_elements
            .iter()
            .find(|element| element.id == id)
    }

    pub fn element_center(&self, element: &MutableElement, visual_size: Vec2) -> Vec2 {
        self.scene.element_center(element, visual_size)
    }
}

pub fn spawn_interior(commands: &mut Commands, asset_server: &AssetServer, map: &InteriorMap) {
    commands.spawn((
        Sprite::from_color(Color::srgb(0.025, 0.018, 0.014), Vec2::splat(2_500.0)),
        Transform::from_xyz(INTERIOR_ORIGIN.x, INTERIOR_ORIGIN.y, -20.0),
        InteriorSceneEntity,
    ));
    let background = spawn_scene_background(commands, asset_server, &map.scene, -10.0);
    commands.entity(background).insert(InteriorSceneEntity);
}

pub fn spawn_building(commands: &mut Commands, asset_server: &AssetServer, map: &MotelExteriorMap) {
    spawn_scene_background(commands, asset_server, &map.scene, -4.5);
}

fn spawn_scene_background(
    commands: &mut Commands,
    asset_server: &AssetServer,
    scene: &SceneMap,
    z: f32,
) -> Entity {
    commands
        .spawn((
            Sprite {
                image: asset_server.load(format!("{}/{}.png", scene.art_directory, scene.id)),
                custom_size: Some(scene.world_size()),
                ..default()
            },
            Transform::from_xyz(scene.origin.x, scene.origin.y, z),
        ))
        .id()
}

pub fn spawn_interior_mutable(
    commands: &mut Commands,
    asset_server: &AssetServer,
    map: &InteriorMap,
    element: &MutableElement,
    state: &str,
) -> Entity {
    spawn_mutable(commands, asset_server, &map.scene, element, state, true)
}

pub fn spawn_building_mutable(
    commands: &mut Commands,
    asset_server: &AssetServer,
    map: &MotelExteriorMap,
    element: &MutableElement,
    state: &str,
) -> Entity {
    spawn_mutable(commands, asset_server, &map.scene, element, state, false)
}

fn spawn_mutable(
    commands: &mut Commands,
    asset_server: &AssetServer,
    scene: &SceneMap,
    element: &MutableElement,
    state: &str,
    interior: bool,
) -> Entity {
    let visual = element
        .states
        .get(state)
        .or_else(|| element.states.get(&element.initial_state))
        .expect("mutable scene element must have its initial visual state");
    let mut sprite = visual.image_path.as_ref().map_or_else(
        || Sprite::from_color(Color::NONE, visual.size.max(Vec2::ONE)),
        |path| Sprite::from_image(asset_server.load(path.clone())),
    );
    sprite.custom_size = Some(visual.size.max(Vec2::ONE));
    sprite.flip_x = element.flip_x;
    sprite.flip_y = element.flip_y;
    let z = if interior {
        match element.layer.as_str() {
            "floor" => -9.0,
            "wall" => -8.0,
            "overlay" => 4.0,
            _ => -3.0,
        }
    } else {
        match element.layer.as_str() {
            "floor" => -4.0,
            "wall" => -3.0,
            "overlay" => 0.0,
            _ => -1.0,
        }
    };
    let center = scene.element_center(element, visual.size);
    let entity = commands
        .spawn((
            sprite,
            Transform::from_xyz(center.x, center.y, z),
            if visual.visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
        ))
        .id();
    if interior {
        commands.entity(entity).insert(InteriorSceneEntity);
    }
    entity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_authored_motel_interior_has_a_walkable_entry_and_exit() {
        for interior_id in InteriorId::ALL {
            let room = InteriorMap::load(interior_id);
            assert_eq!(room.id(), interior_id.id());
            assert!(room.is_walkable(room.cell_center(room.entry)));
            assert_eq!(room.exits.len(), 1);
            assert!(room
                .scene
                .cell_at(room.cell_center(room.exits[0]))
                .is_some());
            assert!(!room.is_walkable(INTERIOR_ORIGIN + room.world_size()));
        }
    }

    #[test]
    fn authored_rooms_keep_repairable_elements_at_native_pixel_size() {
        for interior_id in InteriorId::ALL {
            let room = InteriorMap::load(interior_id);
            let element = room
                .mutable_elements()
                .first()
                .expect("every motel interior has repairable elements");
            assert_eq!(element.initial_state, "damaged");
            assert!(element.states["damaged"].size.cmpgt(Vec2::ZERO).all());
            assert!(element.states.contains_key("repaired"));
        }
    }

    #[test]
    fn motel_exterior_has_seven_authored_doors_in_left_to_right_order() {
        let motel = MotelExteriorMap::load();
        let mut door_x = motel
            .mutable_elements()
            .iter()
            .filter(|element| element.kind == "door")
            .map(|element| element.pixel_x)
            .collect::<Vec<_>>();
        door_x.sort_by(f32::total_cmp);
        assert_eq!(door_x.len(), InteriorId::ALL.len());
        assert!(door_x.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn mutable_instance_deserializes_shared_state_flips() {
        let instance: MutableInstanceDefinition = serde_json::from_str(
            r#"{
                "id": "chair-01",
                "template": "chair",
                "position": {"grid": 16, "x": 3, "y": 5},
                "layer": "overlay",
                "initial_state": "damaged",
                "transform": {"flip_x": true, "flip_y": true}
            }"#,
        )
        .expect("flipped mutable instance");

        assert!(instance.transform.flip_x);
        assert!(instance.transform.flip_y);
        assert_eq!(instance.layer.as_deref(), Some("overlay"));
        assert_eq!(instance.resolved_layer("object"), "overlay");
        assert_eq!(instance.pixel_position(32.0), Vec2::new(48.0, 80.0));
    }

    #[test]
    fn debris_uses_a_no_tool_cleaning_task_by_default() {
        let room = InteriorMap::load(InteriorId::Office);
        let debris = room
            .mutable_elements()
            .iter()
            .find(|element| element.kind == "debris")
            .expect("office has authored debris");

        assert_eq!(debris.task.action, crate::progression::TaskAction::Clean);
        assert!(debris.task.tools.is_empty());
        assert!(debris.task.supplies.is_empty());
    }
}
