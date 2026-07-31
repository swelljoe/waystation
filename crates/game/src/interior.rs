//! Authored interior and building metadata, positioning, and runtime art.

use std::collections::HashMap;

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

impl GridDefinition {
    fn world_size(&self) -> Vec2 {
        Vec2::new(
            f32::from(self.width) * f32::from(self.tile_size),
            f32::from(self.height) * f32::from(self.tile_size),
        )
    }
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
    collision: Vec<CollisionDefinition>,
    #[serde(default)]
    interactions: Vec<InteractionDefinition>,
    #[serde(default)]
    placements: Vec<BakedPlacementDefinition>,
    #[serde(default)]
    templates: HashMap<String, ElementTemplateDefinition>,
    #[serde(default)]
    structures: Vec<MutableInstanceDefinition>,
    #[serde(default)]
    fixtures: Vec<MutableInstanceDefinition>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SceneInteractionKind {
    Search,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SceneDiscovery {
    GideonBible,
}

#[derive(Debug, Deserialize)]
struct InteractionDefinition {
    id: String,
    label: String,
    kind: SceneInteractionKind,
    discovery: SceneDiscovery,
    position: PixelPositionDefinition,
    width: u16,
    height: u16,
}

impl InteractionDefinition {
    #[allow(clippy::cast_precision_loss)]
    fn resolve(self, world_size: Vec2, origin: Vec2) -> SceneInteraction {
        let grid = f32::from(self.position.grid);
        let size = Vec2::new(f32::from(self.width), f32::from(self.height));
        let top_left = Vec2::new(
            (self.position.x as f32).mul_add(grid, origin.x - world_size.x / 2.0),
            (self.position.y as f32).mul_add(-grid, origin.y + world_size.y / 2.0),
        );
        SceneInteraction {
            id: self.id,
            label: self.label,
            kind: self.kind,
            discovery: self.discovery,
            center: top_left + Vec2::new(size.x / 2.0, -size.y / 2.0),
            size,
        }
    }
}

fn resolve_interactions(
    interactions: Vec<InteractionDefinition>,
    world_size: Vec2,
    origin: Vec2,
) -> Vec<SceneInteraction> {
    interactions
        .into_iter()
        .map(|interaction| interaction.resolve(world_size, origin))
        .collect()
}

#[derive(Clone, Debug)]
pub struct SceneInteraction {
    pub id: String,
    pub label: String,
    pub kind: SceneInteractionKind,
    pub discovery: SceneDiscovery,
    pub center: Vec2,
    pub size: Vec2,
}

#[derive(Debug, Deserialize)]
struct BakedPlacementDefinition {
    #[serde(default)]
    x: Option<i16>,
    #[serde(default)]
    y: Option<i16>,
    #[serde(default)]
    position: Option<PixelPositionDefinition>,
    width: u16,
    height: u16,
    #[serde(default)]
    repeat: bool,
    #[serde(default)]
    occludes_player: bool,
    source: SourceDefinition,
}

impl BakedPlacementDefinition {
    #[allow(clippy::cast_precision_loss)]
    fn pixel_position(&self, tile_size: f32) -> Vec2 {
        self.position.as_ref().map_or_else(
            || {
                Vec2::new(
                    f32::from(self.x.expect("legacy placement needs x")) * tile_size,
                    f32::from(self.y.expect("legacy placement needs y")) * tile_size,
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

    fn pixel_size(&self, tile_size: f32) -> Vec2 {
        if self.repeat {
            Vec2::new(
                f32::from(self.width) * tile_size,
                f32::from(self.height) * tile_size,
            )
        } else {
            Vec2::new(
                f32::from(self.source.width) * f32::from(self.source.grid),
                f32::from(self.source.height) * f32::from(self.source.grid),
            )
        }
    }

    fn resolve_occlusion_area(
        &self,
        tile_size: f32,
        world_size: Vec2,
        origin: Vec2,
    ) -> CollisionArea {
        let size = self.pixel_size(tile_size);
        let pixel_position = self.pixel_position(tile_size);
        let top_left = Vec2::new(
            pixel_position.x + origin.x - world_size.x / 2.0,
            -pixel_position.y + origin.y + world_size.y / 2.0,
        );
        CollisionArea {
            center: top_left + Vec2::new(size.x / 2.0, -size.y / 2.0),
            size,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct CollisionDefinition {
    x: i16,
    y: i16,
    #[serde(default)]
    grid: Option<u16>,
}

#[derive(Clone, Copy, Debug)]
struct CollisionArea {
    center: Vec2,
    size: Vec2,
}

impl CollisionDefinition {
    fn resolve(self, default_grid: u16, world_size: Vec2, origin: Vec2) -> CollisionArea {
        let grid = f32::from(self.grid.unwrap_or(default_grid));
        let top_left = Vec2::new(
            f32::from(self.x).mul_add(grid, origin.x - world_size.x / 2.0),
            f32::from(self.y).mul_add(-grid, origin.y + world_size.y / 2.0),
        );
        CollisionArea {
            center: top_left + Vec2::new(grid / 2.0, -grid / 2.0),
            size: Vec2::splat(grid),
        }
    }
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
    #[serde(default)]
    occludes_player: bool,
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
    pub occludes_player: bool,
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
    collision: Vec<CollisionArea>,
    crown_occluders: Vec<CollisionArea>,
    interactions: Vec<SceneInteraction>,
    mutable_elements: Vec<MutableElement>,
}

fn resolve_scene_areas(
    definition: &SceneDefinition,
    world_size: Vec2,
    origin: Vec2,
) -> (Vec<CollisionArea>, Vec<CollisionArea>) {
    let collision = definition
        .collision
        .iter()
        .map(|&area| area.resolve(definition.grid.tile_size, world_size, origin))
        .collect();
    let crown_occluders = definition
        .placements
        .iter()
        .filter(|placement| placement.occludes_player)
        .map(|placement| {
            placement.resolve_occlusion_area(
                f32::from(definition.grid.tile_size),
                world_size,
                origin,
            )
        })
        .collect();
    (collision, crown_occluders)
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
        let world_size = definition.grid.world_size();
        let (collision, crown_occluders) = resolve_scene_areas(&definition, world_size, origin);
        let interactions = resolve_interactions(definition.interactions, world_size, origin);
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
                    occludes_player: instance.occludes_player,
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
                collision,
                crown_occluders,
                interactions,
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

    fn contains_area(&self, center: Vec2, size: Vec2) -> bool {
        let scene_half = self.world_size() / 2.0;
        let area_half = size / 2.0;
        let offset = center - self.origin;
        offset.x.abs() + area_half.x <= scene_half.x && offset.y.abs() + area_half.y <= scene_half.y
    }

    fn area_avoids_collision(&self, center: Vec2, size: Vec2) -> bool {
        let half = size / 2.0;
        self.collision.iter().all(|area| {
            let area_half = area.size / 2.0;
            (center.x - area.center.x).abs() >= half.x + area_half.x
                || (center.y - area.center.y).abs() >= half.y + area_half.y
        })
    }

    fn exterior_depth_ground_y(&self) -> f32 {
        self.collision
            .iter()
            .map(|area| area.center.y - area.size.y / 2.0)
            .min_by(f32::total_cmp)
            .unwrap_or_else(|| self.origin.y - self.world_size().y / 2.0)
    }

    fn contains_exterior_occlusion_point(&self, point: Vec2) -> bool {
        let half_size = self.world_size() / 2.0;
        let ground_y = self.exterior_depth_ground_y();
        point.x >= self.origin.x - half_size.x
            && point.x <= self.origin.x + half_size.x
            && point.y >= ground_y
            && point.y <= self.origin.y + half_size.y
    }

    fn crown_is_fully_occluded(&self, center: Vec2, size: Vec2) -> bool {
        let half_size = size / 2.0;
        self.crown_occluders.iter().any(|area| {
            let area_half_size = area.size / 2.0;
            (center.x - area.center.x).abs() < half_size.x + area_half_size.x
                && (center.y - area.center.y).abs() < half_size.y + area_half_size.y
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

    pub fn is_area_walkable(&self, center: Vec2, size: Vec2) -> bool {
        self.scene.contains_area(center, size) && self.scene.area_avoids_collision(center, size)
    }

    pub fn is_exit(&self, position: Vec2) -> bool {
        self.scene
            .cell_at(position)
            .is_some_and(|cell| self.exits.contains(&cell))
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

    pub fn interactions(&self) -> &[SceneInteraction] {
        &self.scene.interactions
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

    pub fn is_walkable(&self, position: Vec2) -> bool {
        self.scene.area_avoids_collision(position, Vec2::ZERO)
    }

    /// Placement-time counterpart to `is_walkable`: no part of a prop may
    /// overlap an authored building collision cell.
    pub fn is_area_walkable(&self, center: Vec2, size: Vec2) -> bool {
        self.scene.area_avoids_collision(center, size)
    }

    /// Tall exterior art is depth-sorted at the southernmost edge of its
    /// authored collision. This treats the collision footprint as the point
    /// where the building meets the ground, independently of roof overhangs.
    pub fn depth_ground_y(&self) -> f32 {
        self.scene.exterior_depth_ground_y()
    }

    pub fn occludes_ground_point(&self, point: Vec2) -> bool {
        self.scene.contains_exterior_occlusion_point(point)
    }

    pub fn fully_occludes_crown(&self, center: Vec2, size: Vec2) -> bool {
        self.scene.crown_is_fully_occluded(center, size)
    }
}

impl MutableElement {
    pub fn fully_occludes_player(&self) -> bool {
        self.occludes_player || self.kind == "chimney"
    }
}

pub fn spawn_interior(commands: &mut Commands, asset_server: &AssetServer, map: &InteriorMap) {
    commands.spawn((
        Sprite::from_color(Color::srgb(0.025, 0.018, 0.014), Vec2::splat(2_500.0)),
        Transform::from_xyz(INTERIOR_ORIGIN.x, INTERIOR_ORIGIN.y, -20.0),
        InteriorSceneEntity,
    ));
    for background in spawn_scene_backgrounds(commands, asset_server, &map.scene, true) {
        commands.entity(background).insert(InteriorSceneEntity);
    }
}

pub fn spawn_building(
    commands: &mut Commands,
    asset_server: &AssetServer,
    map: &MotelExteriorMap,
) -> Vec<Entity> {
    spawn_scene_backgrounds(commands, asset_server, &map.scene, false)
}

fn scene_layer_z(layer: &str, interior: bool, mutable: bool) -> f32 {
    let z = if interior {
        match layer {
            "floor" => -10.0,
            "wall" => -8.0,
            "object" => -3.0,
            "overlay" => 4.0,
            _ => panic!("unsupported authored scene layer: {layer}"),
        }
    } else {
        match layer {
            "floor" => -4.5,
            "wall" => -3.5,
            "object" => -1.5,
            "overlay" => 0.0,
            _ => panic!("unsupported authored scene layer: {layer}"),
        }
    };
    if mutable {
        z + 0.25
    } else {
        z
    }
}

fn spawn_scene_backgrounds(
    commands: &mut Commands,
    asset_server: &AssetServer,
    scene: &SceneMap,
    interior: bool,
) -> Vec<Entity> {
    ["floor", "wall", "object", "overlay"]
        .into_iter()
        .map(|layer| {
            commands
                .spawn((
                    Sprite {
                        image: asset_server
                            .load(format!("{}/{}--{layer}.png", scene.art_directory, scene.id)),
                        custom_size: Some(scene.world_size()),
                        ..default()
                    },
                    Transform::from_xyz(
                        scene.origin.x,
                        scene.origin.y,
                        scene_layer_z(layer, interior, false),
                    ),
                ))
                .id()
        })
        .collect()
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
    let z = scene_layer_z(&element.layer, interior, true);
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
            assert!(room.is_area_walkable(room.cell_center(room.entry), Vec2::ZERO));
            assert_eq!(room.exits.len(), 1);
            assert!(room
                .scene
                .cell_at(room.cell_center(room.exits[0]))
                .is_some());
            assert!(!room.is_area_walkable(INTERIOR_ORIGIN + room.world_size(), Vec2::ZERO));
        }
    }

    #[test]
    fn authored_interior_entries_fit_the_player_stance() {
        let player_size = Vec2::new(18.0, 16.0);
        let player_offset = Vec2::new(0.0, -18.0);
        for interior_id in InteriorId::ALL {
            let room = InteriorMap::load(interior_id);
            assert!(
                room.is_area_walkable(room.cell_center(room.entry) + player_offset, player_size)
            );
        }
    }

    #[test]
    fn room_three_authors_the_bible_nightstand_search_at_native_size() {
        let room = InteriorMap::load(InteriorId::Room03);
        let [interaction] = room.interactions() else {
            panic!("room 3 should have exactly one authored interaction");
        };

        assert_eq!(interaction.id, "bible-nightstand");
        assert_eq!(interaction.label, "dusty nightstand");
        assert_eq!(interaction.kind, SceneInteractionKind::Search);
        assert_eq!(interaction.discovery, SceneDiscovery::GideonBible);
        assert_eq!(
            interaction.center,
            INTERIOR_ORIGIN + Vec2::new(-64.0, 152.0)
        );
        assert_eq!(interaction.size, Vec2::new(32.0, 48.0));
    }

    #[test]
    fn authored_rooms_keep_repairable_elements_at_native_pixel_size() {
        for interior_id in InteriorId::ALL {
            let room = InteriorMap::load(interior_id);
            if interior_id == InteriorId::Room03 {
                assert!(
                    room.mutable_elements().is_empty(),
                    "the preserved Bible room intentionally has nothing to repair"
                );
                continue;
            }
            assert!(!room.mutable_elements().is_empty());
            for element in room.mutable_elements() {
                assert_eq!(element.initial_state, "damaged");
                assert!(element.states["damaged"].size.cmpgt(Vec2::ZERO).all());
                assert!(element.states.contains_key("repaired"));
            }
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
    fn motel_exterior_enforces_authored_collision_only_inside_its_canvas() {
        let motel = MotelExteriorMap::load();
        let blocked = motel
            .scene
            .collision
            .first()
            .copied()
            .expect("motel exterior has collision cells");

        assert!(!motel.is_walkable(blocked.center));
        assert!(motel.is_walkable(MOTEL_EXTERIOR_ORIGIN + motel.scene.world_size()));
    }

    #[test]
    fn motel_exterior_rejects_props_that_overlap_authored_collision() {
        let motel = MotelExteriorMap::load();
        let blocked = motel
            .scene
            .collision
            .first()
            .copied()
            .expect("motel exterior has collision cells");
        let center = blocked.center;

        assert!(!motel.is_area_walkable(center, Vec2::splat(8.0)));
        assert!(motel.is_area_walkable(
            MOTEL_EXTERIOR_ORIGIN + motel.scene.world_size(),
            Vec2::splat(8.0)
        ));
    }

    #[test]
    fn motel_collision_footprint_defines_its_depth_ground_line() {
        let motel = MotelExteriorMap::load();
        let ground_y = motel.depth_ground_y();

        assert!(motel.occludes_ground_point(Vec2::new(0.0, ground_y)));
        assert!(motel.occludes_ground_point(Vec2::new(0.0, ground_y + 1.0)));
        assert!(!motel.occludes_ground_point(Vec2::new(0.0, ground_y - 1.0)));
        assert!(!motel.occludes_ground_point(Vec2::new(10_000.0, ground_y + 1.0)));
    }

    #[test]
    fn motel_gable_and_chimneys_are_authored_full_occluders() {
        let motel = MotelExteriorMap::load();
        assert_eq!(motel.scene.crown_occluders.len(), 4);

        let gable = motel.scene.crown_occluders[0];
        assert!(motel.fully_occludes_crown(gable.center, Vec2::new(24.0, 16.0)));
        assert!(!motel.fully_occludes_crown(
            MOTEL_EXTERIOR_ORIGIN + Vec2::new(600.0, -250.0),
            Vec2::new(24.0, 16.0),
        ));

        let chimneys: Vec<_> = motel
            .mutable_elements()
            .iter()
            .filter(|element| element.kind == "chimney")
            .collect();
        assert_eq!(chimneys.len(), 2);
        assert!(chimneys
            .iter()
            .all(|element| element.fully_occludes_player()));
    }

    #[test]
    fn mixed_collision_grids_resolve_to_native_pixel_areas() {
        let definition: SceneDefinition = serde_json::from_str(
            r#"{
                "schema_version": 4,
                "id": "fine-collision",
                "name": "Fine collision",
                "grid": {"width": 4, "height": 4, "tile_size": 32},
                "collision": [
                    {"x": 1, "y": 1},
                    {"grid": 8, "x": 9, "y": 5}
                ]
            }"#,
        )
        .expect("mixed legacy and fine collision areas");

        assert_eq!(definition.collision[0].grid, None);
        assert_eq!(definition.collision[1].grid, Some(8));
        let world_size = Vec2::splat(128.0);
        let legacy = definition.collision[0].resolve(32, world_size, Vec2::ZERO);
        let fine = definition.collision[1].resolve(32, world_size, Vec2::ZERO);
        assert_eq!(legacy.center, Vec2::new(-16.0, 16.0));
        assert_eq!(legacy.size, Vec2::splat(32.0));
        assert_eq!(fine.center, Vec2::new(12.0, 20.0));
        assert_eq!(fine.size, Vec2::splat(8.0));
    }

    #[test]
    fn authored_interior_exit_cells_are_detectable_as_thresholds() {
        let room = InteriorMap::load(InteriorId::Office);

        assert!(room.is_exit(room.cell_center(room.exits[0])));
        assert!(!room.is_exit(room.cell_center(room.entry)));
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
    fn baked_and_mutable_art_share_the_authored_layer_order() {
        for interior in [false, true] {
            let floor_mutable = scene_layer_z("floor", interior, true);
            let object_baked = scene_layer_z("object", interior, false);
            let object_mutable = scene_layer_z("object", interior, true);
            let overlay_baked = scene_layer_z("overlay", interior, false);

            assert!(floor_mutable < object_baked);
            assert!(object_baked < object_mutable);
            assert!(object_mutable < overlay_baked);
        }
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
