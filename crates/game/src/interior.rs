//! Authored interior metadata and runtime positioning.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use serde::Deserialize;

pub const INTERIOR_ORIGIN: Vec2 = Vec2::new(8_192.0, 0.0);
const MOTEL_ROOM_JSON: &str = include_str!("../../../content/interiors/motel-room-01.json");
const REPAIR_PAIRS_JSON: &str = include_str!("../../../content/repair-pairs.json");

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
struct InteriorDefinition {
    schema_version: u8,
    id: String,
    name: String,
    grid: GridDefinition,
    entry: Cell,
    exits: Vec<Cell>,
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
    pub layer: String,
    pub pixel_x: f32,
    pub pixel_y: f32,
    pub initial_state: String,
    pub flip_x: bool,
    pub flip_y: bool,
    pub states: HashMap<String, ElementVisual>,
}

#[derive(Resource, Debug)]
pub struct InteriorMap {
    pub id: String,
    pub name: String,
    width: u16,
    height: u16,
    tile_size: f32,
    pub entry: Cell,
    pub exits: Vec<Cell>,
    collision: HashSet<Cell>,
    mutable_elements: Vec<MutableElement>,
}

impl InteriorMap {
    pub fn motel_room() -> Self {
        let definition: InteriorDefinition =
            serde_json::from_str(MOTEL_ROOM_JSON).expect("authored motel room must be valid JSON");
        let repair_pairs: RepairPairLibrary = serde_json::from_str(REPAIR_PAIRS_JSON)
            .expect("authored repair-pair library must be valid JSON");
        assert!(
            matches!(definition.schema_version, 1..=4),
            "unsupported interior schema"
        );
        assert_eq!(repair_pairs.schema_version, 1);
        assert_eq!(definition.id, "motel-room-01");
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
                                    "interiors/{room_id}/{}--{state_name}.png",
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
        Self {
            id: definition.id,
            name: definition.name,
            width: definition.grid.width,
            height: definition.grid.height,
            tile_size,
            entry: definition.entry,
            exits: definition.exits,
            collision: definition.collision.into_iter().collect(),
            mutable_elements,
        }
    }

    pub fn world_size(&self) -> Vec2 {
        Vec2::new(
            f32::from(self.width) * self.tile_size,
            f32::from(self.height) * self.tile_size,
        )
    }

    pub fn cell_center(&self, cell: Cell) -> Vec2 {
        let size = self.world_size();
        Vec2::new(
            (f32::from(cell.x) + 0.5).mul_add(self.tile_size, INTERIOR_ORIGIN.x - size.x / 2.0),
            (f32::from(cell.y) + 0.5).mul_add(-self.tile_size, INTERIOR_ORIGIN.y + size.y / 2.0),
        )
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn cell_at(&self, position: Vec2) -> Option<Cell> {
        let size = self.world_size();
        let x = ((position.x - (INTERIOR_ORIGIN.x - size.x / 2.0)) / self.tile_size).floor();
        let y = (((INTERIOR_ORIGIN.y + size.y / 2.0) - position.y) / self.tile_size).floor();
        if x < 0.0 || y < 0.0 || x >= f32::from(self.width) || y >= f32::from(self.height) {
            return None;
        }
        Some(Cell {
            x: x as u16,
            y: y as u16,
        })
    }

    pub fn is_walkable(&self, position: Vec2) -> bool {
        self.cell_at(position)
            .is_some_and(|cell| !self.collision.contains(&cell))
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
        &self.mutable_elements
    }

    pub fn mutable_element(&self, id: &str) -> Option<&MutableElement> {
        self.mutable_elements
            .iter()
            .find(|element| element.id == id)
    }

    pub fn element_center(&self, element: &MutableElement, visual_size: Vec2) -> Vec2 {
        let room_size = self.world_size();
        let top_left = Vec2::new(
            element.pixel_x + INTERIOR_ORIGIN.x - room_size.x / 2.0,
            -element.pixel_y + INTERIOR_ORIGIN.y + room_size.y / 2.0,
        );
        top_left + Vec2::new(visual_size.x / 2.0, -visual_size.y / 2.0)
    }
}

pub fn spawn(commands: &mut Commands, asset_server: &AssetServer, map: &InteriorMap) {
    commands.spawn((
        Sprite::from_color(Color::srgb(0.025, 0.018, 0.014), Vec2::splat(2_500.0)),
        Transform::from_xyz(INTERIOR_ORIGIN.x, INTERIOR_ORIGIN.y, -20.0),
    ));
    commands.spawn((
        Sprite {
            image: asset_server.load(format!("interiors/{}.png", map.id)),
            custom_size: Some(map.world_size()),
            ..default()
        },
        Transform::from_xyz(INTERIOR_ORIGIN.x, INTERIOR_ORIGIN.y, -10.0),
    ));
}

pub fn spawn_mutable(
    commands: &mut Commands,
    asset_server: &AssetServer,
    map: &InteriorMap,
    element: &MutableElement,
    state: &str,
) -> Entity {
    let visual = element
        .states
        .get(state)
        .or_else(|| element.states.get(&element.initial_state))
        .expect("mutable interior element must have its initial visual state");
    let mut sprite = visual.image_path.as_ref().map_or_else(
        || Sprite::from_color(Color::NONE, visual.size.max(Vec2::ONE)),
        |path| Sprite::from_image(asset_server.load(path.clone())),
    );
    sprite.custom_size = Some(visual.size.max(Vec2::ONE));
    sprite.flip_x = element.flip_x;
    sprite.flip_y = element.flip_y;
    let z = match element.layer.as_str() {
        "floor" => -9.0,
        "wall" => -8.0,
        "overlay" => 4.0,
        _ => -3.0,
    };
    commands
        .spawn((
            sprite,
            Transform::from_xyz(
                map.element_center(element, visual.size).x,
                map.element_center(element, visual.size).y,
                z,
            ),
            if visual.visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
        ))
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_room_has_walkable_entry_and_rejects_outside_positions() {
        let room = InteriorMap::motel_room();
        assert!(room.is_walkable(room.cell_center(room.entry)));
        assert!(!room.is_walkable(INTERIOR_ORIGIN + room.world_size()));
    }

    #[test]
    fn authored_exit_is_inside_room() {
        let room = InteriorMap::motel_room();
        assert_eq!(room.exits.len(), 1);
        assert!(room.cell_at(room.cell_center(room.exits[0])).is_some());
    }

    #[test]
    fn authored_mutable_element_has_stable_states_and_native_pixel_size() {
        let room = InteriorMap::motel_room();
        let element = room
            .mutable_elements
            .first()
            .expect("at least one authored repairable element");
        assert_eq!(element.initial_state, "damaged");
        assert!(element.states["damaged"].size.cmpgt(Vec2::ZERO).all());
        assert!(element.states.contains_key("repaired"));
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
}
