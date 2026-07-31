//! The Waystation at the Edge of the Ash — hackathon vertical slice.

#![allow(clippy::needless_pass_by_value)]

mod game_audio;
mod interior;
mod progression;
mod terrain;

use std::{
    collections::{HashMap, VecDeque},
    fmt::Write as _,
    sync::{Arc, Mutex},
};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use progression::{Progression, SupplyId, TaskAction, ToolCondition, ToolId, ToolLocation};
use serde::{Deserialize, Serialize};
use terrain::{MAP_HALF_HEIGHT, MAP_HALF_WIDTH};
use waystation_shared::{
    fixture_response, vignettes, CardRecipe, InterpretRequest, InterpretResponse,
};

const PLAYER_SPEED: f32 = 210.0;
const INTERACT_DISTANCE: f32 = 72.0;
const DOOR_HEAD_PROBE_OFFSET: f32 = 32.0;
const LOCKED_DOOR_BUMP_DISTANCE: f32 = 4.0;
const DEVELOPMENT_PRESENTATION_SCALE: f32 = 2.0;
const INTERIOR_CAMERA_SCALE: f32 = 0.72;
const CAMERA_HALF_WIDTH: f32 = 480.0 / DEVELOPMENT_PRESENTATION_SCALE;
const CAMERA_HALF_HEIGHT: f32 = 270.0 / DEVELOPMENT_PRESENTATION_SCALE;
const ROMAN_FONT_PATH: &str = "fonts/EBGaramond-Variable.ttf";
const EMOJI_FONT_PATH: &str = "fonts/NotoEmoji-Variable.ttf";
const SCRIBE_ATLAS_COLUMNS: u32 = 13;
const SCRIBE_ATLAS_ROWS: u32 = 54;
const SCRIBE_FRAME_SIZE: u32 = 64;
const SCRIBE_FRAME_SIZE_F32: f32 = 64.0;
const SCRIBE_WALK_FRAMES: usize = 9;
const SCRIBE_WALK_SECONDS_PER_FRAME: f32 = 0.11;
const SCRIBE_TOOL_FRAME_SIZE: u32 = 128;
const SCRIBE_TOOL_COLUMNS: u32 = 6;
const SCRIBE_TOOL_ROWS: u32 = 4;
const SCRIBE_TOOL_SECONDS_PER_FRAME: f32 = 0.15;
const SCRIBE_OCCLUSION_CROWN_WIDTH: f32 = 24.0;
const SCRIBE_OCCLUSION_CROWN_HEIGHT: f32 = 16.0;
const TREE_OCCLUSION_SAMPLE_COLUMNS: u16 = 8;
const TREE_OCCLUSION_SAMPLE_ROWS: u16 = 14;
const TREE_OCCLUSION_SAMPLE_SPACING: f32 = 4.0;
const TREE_OCCLUSION_REQUIRED_PERCENT: usize = 96;
const TREE_OPAQUE_ALPHA: f32 = 0.5;
const TREE_PLAYER_CLEARANCE: f32 = 10.0;
// Top-down movement collides at the lower stance; the torso may overlap tall art.
const PLAYER_COLLISION_OFFSET: Vec2 = Vec2::new(0.0, -18.0);
const PLAYER_COLLISION_SIZE: Vec2 = Vec2::new(18.0, 16.0);
const PLAYER_GROUND_OFFSET_Y: f32 = -30.0;
const EXTERIOR_DEPTH_BASE: f32 = 5.0;
const EXTERIOR_DEPTH_PER_Y: f32 = 0.001;
const BUILDING_LAYER_DEPTH_STEP: f32 = 0.000_1;
// Interiors are authored as fixed layer bands rather than a Y-sorted field, so
// the Scribe holds one depth above every band and only flagged scenery climbs
// past them when they stand behind it.
const INTERIOR_PLAYER_DEPTH: f32 = 5.0;
const INTERIOR_OCCLUDER_COVER_DEPTH: f32 = 5.5;
const INTERIOR_OCCLUDER_DEPTH_PER_Y: f32 = 0.000_1;
const DROP_SEARCH_STEP: f32 = terrain::TILE_SIZE;
const DROP_SEARCH_RINGS: i16 = 72;
const BIBLE_STATE_KEY: &str = "motel-room-03/bible-nightstand";
const DISCOVERY_FOUND_STATE: &str = "found";
const BIBLE_ICON_PATH: &str = "ui/bible-32.png";
const STORY_SEEN_STATE: &str = "seen";

const TREE_PLACEMENTS: [(f32, f32, f32); 18] = [
    (-1_980.0, 1_160.0, 190.0),
    (-1_650.0, 780.0, 150.0),
    (-1_420.0, -980.0, 180.0),
    (-1_050.0, 1_220.0, 200.0),
    (-710.0, 420.0, 160.0),
    (-300.0, 430.0, 180.0),
    (100.0, 455.0, 150.0),
    (520.0, 420.0, 200.0),
    (735.0, 190.0, 150.0),
    (700.0, -50.0, 180.0),
    (-725.0, -120.0, 150.0),
    (1_050.0, 920.0, 190.0),
    (1_380.0, -760.0, 170.0),
    (1_720.0, 1_100.0, 210.0),
    (1_960.0, -1_080.0, 180.0),
    (-1_450.0, 840.0, 140.0),
    (-1_180.0, 1_050.0, 150.0),
    (-910.0, 840.0, 150.0),
];

// Loose tinder is easy to gather. Fallen logs and sound boards become the first
// useful stockpile once the Scribe begins restoring the motel.
const KINDLING_PICKUPS: [(Vec2, &str, Vec2); 8] = [
    (
        Vec2::new(-390.0, -80.0),
        "world/kindling_logs.png",
        Vec2::new(48.0, 34.0),
    ),
    (
        Vec2::new(-285.0, 170.0),
        "world/kindling_branches.png",
        Vec2::new(48.0, 32.0),
    ),
    (
        Vec2::new(-80.0, 355.0),
        "world/kindling_tinder.png",
        Vec2::new(46.0, 30.0),
    ),
    (
        Vec2::new(-980.0, 610.0),
        "world/kindling_branches.png",
        Vec2::new(48.0, 32.0),
    ),
    (
        Vec2::new(-1_380.0, -420.0),
        "world/kindling_tinder.png",
        Vec2::new(46.0, 30.0),
    ),
    (
        Vec2::new(1_030.0, 690.0),
        "world/kindling_logs.png",
        Vec2::new(48.0, 34.0),
    ),
    (
        Vec2::new(1_510.0, -720.0),
        "world/kindling_branches.png",
        Vec2::new(48.0, 32.0),
    ),
    (
        Vec2::new(1_920.0, 880.0),
        "world/kindling_tinder.png",
        Vec2::new(46.0, 30.0),
    ),
];
const LOG_PICKUPS: [Vec2; 6] = [
    Vec2::new(-1_720.0, 930.0),
    Vec2::new(-1_180.0, -920.0),
    Vec2::new(-820.0, 820.0),
    Vec2::new(920.0, -890.0),
    Vec2::new(1_340.0, 1_070.0),
    Vec2::new(1_840.0, -350.0),
];
const PLANK_PICKUPS: [Vec2; 3] = [
    Vec2::new(625.0, -175.0),
    Vec2::new(-1_260.0, 360.0),
    Vec2::new(1_640.0, 520.0),
];
/// The office desk drawer holds the last nails anyone left behind; every nail
/// after that is pulled back out of cleared debris.
const DESK_DRAWER_NAILS: u16 = 12;
/// The first fire the valley has seen in generations.
const HEARTH_KINDLING: u16 = 3;

/// Stone lies where the ash-scoured rim broke, away from the motel court, so
/// masonry costs a walk out to the valley's edges and back.
const STONE_OUTCROP_PLACEMENTS: [(f32, f32); 24] = [
    (-2_060.0, 620.0),
    (-1_880.0, -420.0),
    (-1_640.0, 1_320.0),
    (-1_530.0, -1_180.0),
    (-1_240.0, 300.0),
    (-980.0, -1_320.0),
    (-760.0, 1_240.0),
    (-460.0, -1_090.0),
    (-240.0, 880.0),
    (180.0, -1_240.0),
    (430.0, 1_180.0),
    (760.0, -1_060.0),
    (1_020.0, 380.0),
    (1_180.0, -1_300.0),
    (1_460.0, 900.0),
    (1_620.0, -560.0),
    (1_880.0, 1_260.0),
    (2_040.0, 240.0),
    (2_120.0, -940.0),
    (-2_140.0, -1_340.0),
    (-1_320.0, -680.0),
    (620.0, 1_360.0),
    (1_360.0, 320.0),
    (-560.0, -1_420.0),
];
const STONE_OUTCROP_SIZE: Vec2 = Vec2::new(96.0, 96.0);
/// The sawbuck stands in the court where the Scribe can see both the road and
/// the office door.
const SAWBUCK_POSITION: Vec2 = Vec2::new(390.0, -300.0);
const SAWBUCK_SIZE: Vec2 = Vec2::new(120.0, 96.0);

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    run_game();
}

#[cfg(target_arch = "wasm32")]
fn main() {}

/// Browsers only permit a page to create audible playback during a user gesture.
/// The web shell calls this exported entry point from its start button rather than
/// constructing Bevy's audio output while the page is loading.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn start_web_game() {
    run_game();
}

fn run_game() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.08, 0.09, 0.08)))
        .insert_resource(UiScale(DEVELOPMENT_PRESENTATION_SCALE))
        .insert_resource(Story::default())
        .insert_resource(InterpretInbox::default())
        .insert_resource(initial_world_location())
        .insert_resource(MotelAccess::default())
        .insert_resource(Progression::default())
        .insert_resource(ExteriorReturn::default())
        .init_resource::<NarrativePopup>()
        .init_resource::<DoorwayAttempt>()
        .init_resource::<DoorBumpLatch>()
        .init_resource::<terrain::TerrainDebugOverlay>()
        .init_resource::<InteriorState>()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "The Waystation at the Edge of the Ash".to_owned(),
                        resolution: (960, 540).into(),
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: asset_root().to_owned(),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(game_audio::GameAudioPlugin)
        .add_systems(Update, terrain::update_debug_overlay)
        .add_systems(
            Startup,
            (load_story, setup_world, load_ui_fonts, setup_ui).chain(),
        )
        .add_systems(
            Update,
            (
                move_player,
                handle_automatic_doorways,
                update_exterior_depth,
                update_interior_occlusion,
                animate_player,
                update_player_tree_occlusion,
                sync_player_occlusion_crown
                    .after(update_exterior_depth)
                    .after(animate_player)
                    .after(update_player_tree_occlusion),
                follow_player,
                trigger_story_hotspots,
                sync_portable_tool_entities,
                update_nearby_interaction,
                handle_tool_hotkeys,
                handle_story_input,
                poll_interpretation,
                sync_world_state,
                sync_ui,
                sync_narrative_popup_ui,
                save_story,
            )
                .chain(),
        )
        .add_systems(
            Update,
            handle_interaction
                .after(update_nearby_interaction)
                .before(handle_tool_hotkeys),
        )
        .run();
}

#[cfg(target_arch = "wasm32")]
const fn asset_root() -> &'static str {
    "runtime-assets"
}

#[cfg(target_arch = "wasm32")]
const fn initial_world_location() -> WorldLocation {
    WorldLocation::Exterior
}

#[cfg(not(target_arch = "wasm32"))]
fn initial_world_location() -> WorldLocation {
    if std::env::var_os("WAYSTATION_START_INTERIOR").is_some() {
        WorldLocation::Interior
    } else {
        WorldLocation::Exterior
    }
}

#[cfg(not(target_arch = "wasm32"))]
const fn asset_root() -> &'static str {
    "runtime-assets"
}

#[derive(Component)]
struct Player;

#[derive(Component, Clone, Copy, Debug)]
struct ExteriorYSort {
    ground_offset_y: f32,
    depth_bias: f32,
}

#[derive(Component, Clone, Copy, Debug)]
struct ExteriorFixedDepth {
    ground_y: f32,
    depth_bias: f32,
}

#[derive(Component)]
struct PlayerOcclusionCrown;

#[derive(Component)]
struct BuildingCrownOccluder;

#[derive(Component)]
struct DenseTreeOccluder;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Facing {
    Up,
    Left,
    Down,
    Right,
}

impl Facing {
    const fn walk_row(self) -> usize {
        match self {
            Self::Up => 8,
            Self::Left => 9,
            Self::Down => 10,
            Self::Right => 11,
        }
    }

    const fn direction_index(self) -> usize {
        match self {
            Self::Up => 0,
            Self::Left => 1,
            Self::Down => 2,
            Self::Right => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ToolWorkAnimation {
    Hammer,
    Axe,
}

#[derive(Clone, Copy, Debug)]
struct ActiveToolAnimation {
    kind: ToolWorkAnimation,
    frame: usize,
}

#[derive(Component)]
struct PlayerAnimation {
    timer: Timer,
    facing: Facing,
    frame: usize,
    last_position: Vec2,
    active_tool: Option<ActiveToolAnimation>,
}

#[derive(Resource)]
struct PlayerArt {
    walk_image: Handle<Image>,
    walk_layout: Handle<TextureAtlasLayout>,
    hammer_image: Handle<Image>,
    hammer_layout: Handle<TextureAtlasLayout>,
    axe_image: Handle<Image>,
    axe_layout: Handle<TextureAtlasLayout>,
}

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct Traveler;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum InteractableKind {
    Sign,
    Kindling,
    Log,
    Hearth,
    Plank,
    Tool,
    Desk,
    BibleNightstand,
    Traveler,
    MotelDoor,
    InteriorExit,
    InteriorRepairable,
    ExteriorRepairable,
    Tree,
    Sawbuck,
    StoneOutcrop,
}

#[derive(Component)]
struct WorldPickup {
    id: String,
    reward: PickupReward,
}

#[derive(Clone, Copy)]
enum PickupReward {
    Supply(SupplyId, u16),
}

/// The court's sawbuck: a standing station rather than a pickup, so the valley's
/// fallen wood can keep answering the motel's need for planks.
#[derive(Component)]
struct MillingBench;

#[derive(Component)]
struct StoneOutcrop {
    id: String,
    footprint: ExteriorRect,
    art: ExteriorRect,
}

#[derive(Component)]
struct ChoppableTree {
    id: String,
    trunk: ExteriorRect,
    art: ExteriorRect,
}

#[derive(Component)]
struct PortableToolEntity {
    id: String,
}

#[derive(Clone, Debug)]
struct PortableToolDefinition {
    id: String,
    label: String,
    tool: ToolId,
    condition: ToolCondition,
    layer: String,
    home_scene: String,
    home_position: Vec2,
    image_path: String,
    size: Vec2,
    flip_x: bool,
    flip_y: bool,
}

#[derive(Resource, Default)]
struct PortableToolCatalog(Vec<PortableToolDefinition>);

#[derive(Component, Clone)]
struct TaskTarget {
    action: TaskAction,
    requirements: String,
}

#[derive(Component)]
struct Interactable {
    kind: InteractableKind,
    consumed: bool,
}

#[derive(Component)]
struct AuthoredInteractionLabel(String);

#[derive(Component, Clone, Copy)]
struct StoryHotspot {
    beat: StoryBeat,
    radius: f32,
}

#[derive(Component)]
struct MutableSceneElement {
    scene_id: String,
    id: String,
    state: String,
}

#[derive(Component, Clone, Copy)]
struct MotelDoorDestination {
    interior_id: interior::InteriorId,
    initially_unlocked: bool,
    doorstep: Vec2,
}

#[derive(Resource, Default)]
struct MotelAccess {
    keys_found: bool,
}

#[derive(Resource)]
struct ExteriorReturn(Vec2);

impl Default for ExteriorReturn {
    fn default() -> Self {
        Self(Vec2::new(-570.5, -134.5))
    }
}

#[derive(Resource, Default)]
struct DoorwayAttempt(Option<MotelDoorDestination>);

#[derive(Resource, Default)]
struct DoorBumpLatch(Option<interior::InteriorId>);

#[derive(Resource, Default)]
struct InteriorState(HashMap<String, String>);

#[derive(SystemParam)]
struct InteractionResources<'w> {
    interior_state: ResMut<'w, InteriorState>,
    motel_access: ResMut<'w, MotelAccess>,
    progression: ResMut<'w, Progression>,
}

#[derive(SystemParam)]
struct InteractionQueries<'w, 's> {
    interactables: Query<'w, 's, &'static mut Interactable>,
    pickups: Query<'w, 's, &'static WorldPickup>,
    portable_tools: Query<
        'w,
        's,
        (
            &'static PortableToolEntity,
            &'static AuthoredInteractionLabel,
        ),
    >,
    choppable_trees: Query<'w, 's, &'static ChoppableTree>,
    stone_outcrops: Query<'w, 's, &'static StoneOutcrop>,
    player_animation: Query<'w, 's, &'static mut PlayerAnimation, With<Player>>,
    mutable_elements: Query<
        'w,
        's,
        (
            &'static mut MutableSceneElement,
            &'static mut Sprite,
            &'static mut Transform,
            &'static mut Visibility,
        ),
        Without<Player>,
    >,
}

#[derive(SystemParam)]
struct NarrativeResources<'w> {
    interior_state: ResMut<'w, InteriorState>,
    popup: ResMut<'w, NarrativePopup>,
}

#[derive(SystemParam)]
struct UiKnowledge<'w> {
    interior_state: Res<'w, InteriorState>,
    motel_access: Res<'w, MotelAccess>,
}

#[derive(SystemParam)]
struct MovementEnvironment<'w, 's> {
    location: Res<'w, WorldLocation>,
    interior: Res<'w, interior::InteriorMap>,
    motel: Res<'w, interior::MotelExteriorMap>,
    tool_shed: Res<'w, interior::ToolShedExteriorMap>,
    terrain: Res<'w, terrain::WorldGrid>,
    obstacles: Res<'w, ExteriorObstacles>,
    doors: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static Sprite,
            &'static MotelDoorDestination,
        ),
        Without<Player>,
    >,
    doorway_attempt: ResMut<'w, DoorwayAttempt>,
}

#[derive(SystemParam)]
struct ToolDropEnvironment<'w> {
    location: Res<'w, WorldLocation>,
    interior: Res<'w, interior::InteriorMap>,
    terrain: Res<'w, terrain::WorldGrid>,
    motel: Res<'w, interior::MotelExteriorMap>,
    tool_shed: Res<'w, interior::ToolShedExteriorMap>,
    obstacles: Res<'w, ExteriorObstacles>,
    catalog: Res<'w, PortableToolCatalog>,
}

#[derive(Clone, Copy, Debug)]
struct ExteriorRect {
    center: Vec2,
    size: Vec2,
}

impl ExteriorRect {
    const fn new(center: Vec2, size: Vec2) -> Self {
        Self { center, size }
    }

    fn overlaps(self, other: Self) -> bool {
        let reach = (self.size + other.size) / 2.0;
        (self.center.x - other.center.x).abs() < reach.x
            && (self.center.y - other.center.y).abs() < reach.y
    }
}

fn same_exterior_rect(left: ExteriorRect, right: ExteriorRect) -> bool {
    left.center == right.center && left.size == right.size
}

/// Ground-level obstruction for everything the valley grows or the Scribe
/// builds: tree trunks, the sawbuck, quarried outcrops. `solid_footprints`
/// blocks movement; `prop_exclusions` is the wider art bound that keeps later
/// pickups from spawning underneath something.
#[derive(Resource, Default, Debug)]
struct ExteriorObstacles {
    solid_footprints: Vec<ExteriorRect>,
    prop_exclusions: Vec<ExteriorRect>,
}

impl ExteriorObstacles {
    fn player_can_stand(&self, bounds: ExteriorRect) -> bool {
        self.solid_footprints
            .iter()
            .all(|obstacle| !obstacle.overlaps(bounds))
    }

    fn prop_is_clear(&self, bounds: ExteriorRect) -> bool {
        self.prop_exclusions
            .iter()
            .all(|obstacle| !obstacle.overlaps(bounds))
    }
}

#[derive(Component)]
struct StatusText;

#[derive(Component)]
struct PromptText;

#[derive(Component)]
struct OverlayRoot;

#[derive(Component)]
struct OverlayTitle;

#[derive(Component)]
struct OverlayBody;

#[derive(Component)]
struct ProvenanceText;

#[derive(Component)]
struct ProgressText;

#[derive(Component)]
struct CardArt;

#[derive(Component)]
struct NarrativePopupRoot;

#[derive(Component)]
struct NarrativePopupTitle;

#[derive(Component)]
struct NarrativePopupBody;

#[derive(Component)]
struct NarrativePopupArt;

#[derive(Component)]
struct NarrativePopupDismiss;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiscoveredItem {
    GideonBible,
}

impl DiscoveredItem {
    const fn title(self) -> &'static str {
        match self {
            Self::GideonBible => "A Small Book",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::GideonBible => {
                "This must be a book… I've seen paper before, but never so much of it bound together. The pages are thin as onion skin—and somehow still dry. Someone left it here for a stranger.\n\nIt has waited safely in this room for generations. It is too precious for my leaky old pack. I'll leave it here, read while the storm passes, and come back when it is time to go."
            }
        }
    }

    const fn image_path(self) -> &'static str {
        match self {
            Self::GideonBible => BIBLE_ICON_PATH,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StoryBeat {
    OfficeThreshold,
    OfficeHearth,
    OfficeLedger,
    OfficeWelcome,
    RoomThreePreserved,
}

impl StoryBeat {
    const fn title(self) -> &'static str {
        match self {
            Self::OfficeThreshold => "The Unnumbered Door",
            Self::OfficeHearth => "A Place to Come In",
            Self::OfficeLedger => "Names and Numbers",
            Self::OfficeWelcome => "For Strangers",
            Self::RoomThreePreserved => "Room Three",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::OfficeThreshold => {
                "The door at the left had no number. The six beside it did. This room is not for sleeping: a desk faces the entrance, and someone once sat here waiting for whoever came through it."
            }
            Self::OfficeHearth => {
                "Two chairs have been drawn close to the hearth—not a private room, then. A place to come in wet and cold. A place where someone was expected."
            }
            Self::OfficeLedger => {
                "Scraps of ruled paper cling together beneath the desk's shallow drawer. Names. Dates. Numbers matching the doors outside. A guest ledger.\n\nThe brass keys carry those same numbers. I don't need to carry them far; they belong here, and now I know which doors they open."
            }
            Self::OfficeWelcome => {
                "A room for receiving people. Keys kept ready. Fire and chairs for the road-worn.\n\nThis was a place that welcomed strangers."
            }
            Self::RoomThreePreserved => {
                "The glass didn't fail. The door swelled shut, and the roof must have held. Decades of dust—perhaps a century—but nothing here is broken.\n\nHow did this one room endure when all the others opened to the weather?"
            }
        }
    }

    const fn state_key(self) -> &'static str {
        match self {
            Self::OfficeThreshold => "story/office-threshold",
            Self::OfficeHearth => "story/office-hearth",
            Self::OfficeLedger => "story/office-ledger",
            Self::OfficeWelcome => "story/office-welcome",
            Self::RoomThreePreserved => "story/room-three-preserved",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NarrativeCard {
    Item(DiscoveredItem),
    Thought(StoryBeat),
}

impl NarrativeCard {
    const fn title(self) -> &'static str {
        match self {
            Self::Item(item) => item.title(),
            Self::Thought(beat) => beat.title(),
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::Item(item) => item.description(),
            Self::Thought(beat) => beat.description(),
        }
    }

    const fn image_path(self) -> Option<&'static str> {
        match self {
            Self::Item(item) => Some(item.image_path()),
            Self::Thought(_) => None,
        }
    }

    const fn dismiss_label(self) -> &'static str {
        match self {
            Self::Item(DiscoveredItem::GideonBible) => "E / SPACE — leave it safe here",
            Self::Thought(_) => "E / SPACE — continue",
        }
    }
}

#[derive(Resource, Default)]
struct NarrativePopup {
    current: Option<NarrativeCard>,
    queue: VecDeque<NarrativeCard>,
    dismiss_armed: bool,
}

impl NarrativePopup {
    fn present(&mut self, card: NarrativeCard) {
        if self.current == Some(card) || self.queue.contains(&card) {
            return;
        }
        if self.current.is_some() {
            self.queue.push_back(card);
            return;
        }
        self.current = Some(card);
        self.dismiss_armed = false;
    }

    const fn is_open(&self) -> bool {
        self.current.is_some()
    }

    /// The interaction key which opens a discovery must not also close it in
    /// the same update. The first input pass arms dismissal; a later E, Space,
    /// or Escape accepts the item.
    fn handle_input(&mut self, dismiss_pressed: bool) {
        if self.current.is_none() {
            return;
        }
        if !self.dismiss_armed {
            self.dismiss_armed = true;
        } else if dismiss_pressed {
            self.current = self.queue.pop_front();
            self.dismiss_armed = false;
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
enum WorldLocation {
    Exterior,
    Interior,
}

#[derive(Resource)]
struct UiFonts {
    roman: Handle<Font>,
    emoji: Handle<Font>,
}

impl UiFonts {
    fn roman(&self, font_size: f32) -> TextFont {
        TextFont {
            font: self.roman.clone(),
            font_size,
            ..default()
        }
    }

    fn emoji(&self, font_size: f32) -> TextFont {
        TextFont {
            font: self.emoji.clone(),
            font_size,
            ..default()
        }
    }
}

#[derive(Resource, Default)]
struct Nearby(Option<Entity>);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum StoryStage {
    Arrival,
    GatherKindling,
    LightHearth,
    FindBible,
    FindPlank,
    RestoreDesk,
    Night,
    MeetTraveler,
    Dialogue,
    Interpreting,
    ChoosePaper,
    ChooseIllustration,
    ChooseBorder,
    FinishedCard,
    Epilogue,
}

#[derive(Resource)]
struct Story {
    stage: StoryStage,
    kindling: u8,
    vignette_index: usize,
    dialogue_line: usize,
    result: Option<InterpretResponse>,
    card: CardRecipe,
    notice: Option<String>,
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Debug, Serialize, Deserialize)]
struct SaveData {
    version: u8,
    stage: StoryStage,
    kindling: u8,
    vignette_index: usize,
    dialogue_line: usize,
    result: Option<InterpretResponse>,
    card: CardRecipe,
    #[serde(default)]
    interior_states: HashMap<String, String>,
    #[serde(default)]
    motel_keys_found: bool,
    #[serde(default)]
    progression: Progression,
}

impl SaveData {
    #[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
    fn capture(
        story: &Story,
        interior_state: &InteriorState,
        motel_access: &MotelAccess,
        progression: &Progression,
    ) -> Self {
        Self {
            version: 6,
            stage: story.stage,
            kindling: story.kindling,
            vignette_index: story.vignette_index,
            dialogue_line: story.dialogue_line,
            result: story.result.clone(),
            card: story.card.clone(),
            interior_states: interior_state.0.clone(),
            motel_keys_found: motel_access.keys_found,
            progression: progression.clone(),
        }
    }
}

impl Default for Story {
    fn default() -> Self {
        Self {
            stage: StoryStage::Arrival,
            kindling: 0,
            vignette_index: 0,
            dialogue_line: 0,
            result: None,
            card: CardRecipe::default(),
            notice: Some(
                "The storm has followed you for two days. Then, below the ridge: stone walls."
                    .to_owned(),
            ),
        }
    }
}

impl Story {
    fn vignette_id(&self) -> &'static str {
        vignettes()[self.vignette_index % vignettes().len()]
            .id
            .as_str()
    }

    fn traveler_name(&self) -> &'static str {
        vignettes()[self.vignette_index % vignettes().len()]
            .traveler_name
            .as_str()
    }

    fn reset_for_replay(&mut self) {
        self.stage = StoryStage::Arrival;
        self.kindling = 0;
        self.vignette_index = (self.vignette_index + 1) % vignettes().len();
        self.dialogue_line = 0;
        self.result = None;
        self.card = CardRecipe::default();
        self.notice = Some("Another telling begins at the valley rim.".to_owned());
    }
}

type InboxValue = Option<Result<InterpretResponse, String>>;

#[derive(Resource, Clone, Default)]
struct InterpretInbox(Arc<Mutex<InboxValue>>);

#[allow(clippy::too_many_lines)]
fn setup_world(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    location: Res<WorldLocation>,
    mut interior_state: ResMut<InteriorState>,
    mut popup: ResMut<NarrativePopup>,
    mut progression: ResMut<Progression>,
) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scale: DEVELOPMENT_PRESENTATION_SCALE.recip(),
            ..OrthographicProjection::default_2d()
        }),
        MainCamera,
    ));
    let world_grid =
        terrain::spawn_terrain(&mut commands, &asset_server, &mut texture_atlas_layouts);
    commands.insert_resource(world_grid.clone());
    let tree = asset_server.load("world/tree.png");
    let scribe = asset_server.load("world/scribe.png");
    let scribe_hammer = asset_server.load("world/scribe-hammer.png");
    let scribe_axe = asset_server.load("world/scribe-axe.png");
    let motel = interior::MotelExteriorMap::load();
    let building_ground_y = motel.depth_ground_y();
    for (layer_index, entity) in interior::spawn_building(&mut commands, &asset_server, &motel)
        .into_iter()
        .enumerate()
    {
        commands.entity(entity).insert(ExteriorFixedDepth {
            ground_y: building_ground_y,
            depth_bias: building_layer_depth_bias(layer_index, false),
        });
    }
    let door_routes = motel_door_routes(&motel);
    for element in motel.mutable_elements() {
        let state_key = format!("{}/{}", motel.id(), element.id);
        let state = interior_state
            .0
            .get(&state_key)
            .map_or(element.initial_state.as_str(), String::as_str);
        let entity =
            interior::spawn_building_mutable(&mut commands, &asset_server, &motel, element, state);
        commands.entity(entity).insert(ExteriorFixedDepth {
            ground_y: building_ground_y,
            depth_bias: building_layer_depth_bias(authored_layer_index(&element.layer), true),
        });
        if element.fully_occludes_player() {
            commands.entity(entity).insert(BuildingCrownOccluder);
        }
        let kind = if door_routes.contains_key(&element.id) {
            InteractableKind::MotelDoor
        } else {
            InteractableKind::ExteriorRepairable
        };
        commands.entity(entity).insert((
            Interactable {
                kind,
                consumed: kind != InteractableKind::MotelDoor && state == "repaired",
            },
            MutableSceneElement {
                scene_id: motel.id().to_owned(),
                id: element.id.clone(),
                state: state.to_owned(),
            },
        ));
        if kind != InteractableKind::MotelDoor {
            commands.entity(entity).insert(TaskTarget {
                action: element.task.action,
                requirements: element.task.requirements_text(),
            });
        }
        if let Some(&interior_id) = door_routes.get(&element.id) {
            let visual = element
                .states
                .get(state)
                .or_else(|| element.states.get(&element.initial_state))
                .expect("door needs its authored visual");
            let doorstep = motel.element_center(element, visual.size) + Vec2::new(0.0, -52.0);
            commands.entity(entity).insert(MotelDoorDestination {
                interior_id,
                initially_unlocked: matches!(
                    interior_id,
                    interior::InteriorId::Office | interior::InteriorId::Room05
                ),
                doorstep,
            });
        }
    }

    let tool_shed = interior::ToolShedExteriorMap::load();
    let tool_shed_ground_y = tool_shed.depth_ground_y();
    for (layer_index, entity) in
        interior::spawn_tool_shed_building(&mut commands, &asset_server, &tool_shed)
            .into_iter()
            .enumerate()
    {
        commands.entity(entity).insert(ExteriorFixedDepth {
            ground_y: tool_shed_ground_y,
            depth_bias: building_layer_depth_bias(layer_index, false),
        });
    }
    for element in tool_shed.mutable_elements() {
        let state_key = format!("{}/{}", tool_shed.id(), element.id);
        let state = interior_state
            .0
            .get(&state_key)
            .map_or(element.initial_state.as_str(), String::as_str);
        let entity = interior::spawn_tool_shed_mutable(
            &mut commands,
            &asset_server,
            &tool_shed,
            element,
            state,
        );
        commands.entity(entity).insert(ExteriorFixedDepth {
            ground_y: tool_shed_ground_y,
            depth_bias: building_layer_depth_bias(authored_layer_index(&element.layer), true),
        });
        if element.fully_occludes_player() {
            commands.entity(entity).insert(BuildingCrownOccluder);
        }
        let is_door = element.kind == "door";
        let kind = if is_door {
            InteractableKind::MotelDoor
        } else {
            InteractableKind::ExteriorRepairable
        };
        commands.entity(entity).insert((
            Interactable {
                kind,
                consumed: !is_door && state == "repaired",
            },
            MutableSceneElement {
                scene_id: tool_shed.id().to_owned(),
                id: element.id.clone(),
                state: state.to_owned(),
            },
        ));
        if is_door {
            let visual = element
                .states
                .get(state)
                .or_else(|| element.states.get(&element.initial_state))
                .expect("tool-shed door needs its authored visual");
            let doorstep = tool_shed.element_center(element, visual.size) + Vec2::new(0.0, -52.0);
            commands.entity(entity).insert(MotelDoorDestination {
                interior_id: interior::InteriorId::ToolShed,
                initially_unlocked: true,
                doorstep,
            });
        } else {
            commands.entity(entity).insert(TaskTarget {
                action: element.task.action,
                requirements: element.task.requirements_text(),
            });
        }
    }

    let interior_map = interior::InteriorMap::load(interior::InteriorId::Office);
    if *location == WorldLocation::Interior {
        spawn_interior_scene(&mut commands, &asset_server, &interior_map, &interior_state);
        present_story_beat(&mut popup, &mut interior_state, StoryBeat::OfficeThreshold);
    }

    // The motel court is only one clearing in a much larger, forageable valley.
    // A tree's small ground footprint must be fully on land; its broad art bounds
    // keep later forage placement from hiding objects beneath the canopy.
    let mut exterior_obstacles = ExteriorObstacles::default();
    for (index, (x, y, size)) in TREE_PLACEMENTS.into_iter().enumerate() {
        let tree_id = format!("standing-tree-{index:02}");
        if progression.pickup_collected(&tree_id) {
            continue;
        }
        let position =
            resolve_tree_position(&world_grid, &motel, &tool_shed, Vec2::new(x, y), size);
        commands.spawn((
            Sprite {
                image: tree.clone(),
                custom_size: Some(Vec2::splat(size)),
                ..default()
            },
            Transform::from_xyz(
                position.x,
                position.y,
                exterior_depth(size.mul_add(-0.34, position.y)),
            ),
            ExteriorYSort {
                ground_offset_y: -size * 0.34,
                depth_bias: 0.0,
            },
            DenseTreeOccluder,
            Interactable {
                kind: InteractableKind::Tree,
                consumed: false,
            },
            TaskTarget {
                action: TaskAction::Clear,
                requirements: progression::TaskSpec::for_tree_chopping().requirements_text(),
            },
            ChoppableTree {
                id: tree_id,
                trunk: tree_trunk_rect(position, size),
                art: ExteriorRect::new(position, Vec2::splat(size)),
            },
        ));
        exterior_obstacles
            .solid_footprints
            .push(tree_trunk_rect(position, size));
        exterior_obstacles
            .prop_exclusions
            .push(ExteriorRect::new(position, Vec2::splat(size)));
    }

    spawn_interactable(
        &mut commands,
        InteractableKind::Sign,
        Vec2::new(-160.0, -245.0),
        Vec2::new(72.0, 96.0),
        Color::srgb(0.37, 0.24, 0.14),
    );
    // Loose tinder is easy to gather. Fallen logs and sound boards become the
    // first useful stockpile once the Scribe begins restoring the motel.
    let mut pickup_bounds = Vec::new();
    for (index, (position, art, size)) in KINDLING_PICKUPS.into_iter().enumerate() {
        spawn_safe_world_pickup(
            &mut commands,
            &progression,
            &world_grid,
            &motel,
            &tool_shed,
            &exterior_obstacles,
            &mut pickup_bounds,
            format!("kindling-{index:02}"),
            InteractableKind::Kindling,
            position,
            size,
            Sprite::from_image(asset_server.load(art)),
            PickupReward::Supply(SupplyId::Kindling, 1),
        );
    }
    for (index, position) in LOG_PICKUPS.into_iter().enumerate() {
        spawn_safe_world_pickup(
            &mut commands,
            &progression,
            &world_grid,
            &motel,
            &tool_shed,
            &exterior_obstacles,
            &mut pickup_bounds,
            format!("fallen-log-{index:02}"),
            InteractableKind::Log,
            position,
            Vec2::new(64.0, 32.0),
            Sprite::from_image(asset_server.load("world/fallen_log.png")),
            PickupReward::Supply(SupplyId::Log, 1),
        );
    }
    for (index, position) in PLANK_PICKUPS.into_iter().enumerate() {
        spawn_safe_world_pickup(
            &mut commands,
            &progression,
            &world_grid,
            &motel,
            &tool_shed,
            &exterior_obstacles,
            &mut pickup_bounds,
            format!("sound-plank-{index:02}"),
            InteractableKind::Plank,
            position,
            Vec2::new(80.0, 24.0),
            Sprite::from_image(asset_server.load("world/plank.png")),
            PickupReward::Supply(SupplyId::Plank, 1),
        );
    }

    // Standing work stations, not one-time props: the sawbuck answers every
    // later call for planks, and each outcrop carries stone for the masonry the
    // motel is full of.
    let sawbuck_position = safe_pickup_position(
        &world_grid,
        &motel,
        &tool_shed,
        &exterior_obstacles,
        &pickup_bounds,
        SAWBUCK_POSITION,
        SAWBUCK_SIZE,
    );
    pickup_bounds.push(ExteriorRect::new(sawbuck_position, SAWBUCK_SIZE));
    spawn_worked_station(
        &mut commands,
        asset_server.load("world/sawbuck.png"),
        InteractableKind::Sawbuck,
        progression::TaskSpec::for_milling(),
        sawbuck_position,
        SAWBUCK_SIZE,
        &mut exterior_obstacles,
    )
    .insert(MillingBench);
    for (index, (x, y)) in STONE_OUTCROP_PLACEMENTS.into_iter().enumerate() {
        let outcrop_id = format!("stone-outcrop-{index:02}");
        if progression.pickup_collected(&outcrop_id) {
            continue;
        }
        let position = safe_pickup_position(
            &world_grid,
            &motel,
            &tool_shed,
            &exterior_obstacles,
            &pickup_bounds,
            Vec2::new(x, y),
            STONE_OUTCROP_SIZE,
        );
        pickup_bounds.push(ExteriorRect::new(position, STONE_OUTCROP_SIZE));
        spawn_worked_station(
            &mut commands,
            asset_server.load("world/stone_outcrop.png"),
            InteractableKind::StoneOutcrop,
            progression::TaskSpec::for_quarrying(),
            position,
            STONE_OUTCROP_SIZE,
            &mut exterior_obstacles,
        )
        .insert(StoneOutcrop {
            id: outcrop_id,
            footprint: station_footprint(position, STONE_OUTCROP_SIZE),
            art: ExteriorRect::new(position, STONE_OUTCROP_SIZE),
        });
    }

    let tool_shed_interior = interior::InteriorMap::load(interior::InteriorId::ToolShed);
    let mut portable_tools = tool_shed_interior
        .portable_items()
        .iter()
        .map(|item| PortableToolDefinition {
            id: item.id.clone(),
            label: item.label.clone(),
            tool: item.tool,
            condition: item.condition,
            layer: item.layer.clone(),
            home_scene: tool_shed_interior.id().to_owned(),
            home_position: item.center,
            image_path: item.image_path.clone(),
            size: item.size,
            flip_x: item.flip_x,
            flip_y: item.flip_y,
        })
        .collect::<Vec<_>>();
    let ladder_size = Vec2::new(44.0, 112.0);
    let ladder_position = safe_pickup_position(
        &world_grid,
        &motel,
        &tool_shed,
        &exterior_obstacles,
        &pickup_bounds,
        Vec2::new(-1_080.0, 1_010.0),
        ladder_size,
    );
    portable_tools.push(PortableToolDefinition {
        id: "fallen-ladder-01".to_owned(),
        label: "weathered ladder".to_owned(),
        tool: ToolId::Ladder,
        condition: ToolCondition::Serviceable,
        layer: "object".to_owned(),
        home_scene: "exterior".to_owned(),
        home_position: ladder_position,
        image_path: "world/ladder.png".to_owned(),
        size: ladder_size,
        flip_x: false,
        flip_y: false,
    });
    for definition in &portable_tools {
        progression.register_tool_instance(&definition.id, definition.tool, definition.condition);
        spawn_portable_tool_entity(&mut commands, &asset_server, definition);
    }
    commands.insert_resource(PortableToolCatalog(portable_tools));
    commands.insert_resource(exterior_obstacles);
    let traveler = spawn_interactable(
        &mut commands,
        InteractableKind::Traveler,
        Vec2::new(-70.0, -160.0),
        Vec2::new(30.0, 48.0),
        Color::srgb(0.47, 0.30, 0.39),
    );
    commands
        .entity(traveler)
        .insert((Traveler, Visibility::Hidden));

    let player_position = if *location == WorldLocation::Interior {
        interior_map.cell_center(interior_map.entry)
    } else {
        Vec2::new(-650.0, -260.0)
    };
    let scribe_layout = texture_atlas_layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(SCRIBE_FRAME_SIZE),
        SCRIBE_ATLAS_COLUMNS,
        SCRIBE_ATLAS_ROWS,
        None,
        None,
    ));
    let scribe_tool_layout = texture_atlas_layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(SCRIBE_TOOL_FRAME_SIZE),
        SCRIBE_TOOL_COLUMNS,
        SCRIBE_TOOL_ROWS,
        None,
        None,
    ));
    commands.insert_resource(PlayerArt {
        walk_image: scribe.clone(),
        walk_layout: scribe_layout.clone(),
        hammer_image: scribe_hammer,
        hammer_layout: scribe_tool_layout.clone(),
        axe_image: scribe_axe,
        axe_layout: scribe_tool_layout,
    });
    let facing = Facing::Down;
    commands.spawn((
        Sprite {
            image: scribe.clone(),
            texture_atlas: Some(TextureAtlas {
                layout: scribe_layout.clone(),
                index: facing.walk_row() * SCRIBE_ATLAS_COLUMNS as usize,
            }),
            ..default()
        },
        Transform::from_xyz(
            player_position.x,
            player_position.y,
            exterior_depth(player_position.y + PLAYER_GROUND_OFFSET_Y),
        ),
        Player,
        ExteriorYSort {
            ground_offset_y: PLAYER_GROUND_OFFSET_Y,
            depth_bias: 0.0,
        },
        PlayerAnimation {
            timer: Timer::from_seconds(SCRIBE_WALK_SECONDS_PER_FRAME, TimerMode::Repeating),
            facing,
            frame: 0,
            last_position: player_position,
            active_tool: None,
        },
    ));
    commands.spawn((
        Sprite {
            image: scribe,
            texture_atlas: Some(TextureAtlas {
                layout: scribe_layout,
                index: facing.walk_row() * SCRIBE_ATLAS_COLUMNS as usize,
            }),
            rect: Some(Rect::new(
                0.0,
                0.0,
                SCRIBE_FRAME_SIZE_F32,
                SCRIBE_OCCLUSION_CROWN_HEIGHT,
            )),
            ..default()
        },
        Transform::from_xyz(
            player_position.x,
            player_position.y + scribe_occlusion_crown_offset_y(),
            building_occlusion_crown_depth(building_ground_y),
        ),
        PlayerOcclusionCrown,
        Visibility::Hidden,
    ));
    commands.insert_resource(motel);
    commands.insert_resource(tool_shed);
    commands.insert_resource(interior_map);
    commands.insert_resource(Nearby::default());
}

fn spawn_interior_scene(
    commands: &mut Commands,
    asset_server: &AssetServer,
    map: &interior::InteriorMap,
    interior_state: &InteriorState,
) {
    interior::spawn_interior(commands, asset_server, map);
    for occluder in map.occluders() {
        interior::spawn_interior_occluder(commands, asset_server, occluder);
    }
    for element in map.mutable_elements() {
        let state_key = format!("{}/{}", map.id(), element.id);
        let state = interior_state
            .0
            .get(&state_key)
            .map_or(element.initial_state.as_str(), String::as_str);
        let entity = interior::spawn_interior_mutable(commands, asset_server, map, element, state);
        let kind = if map.interior_id == interior::InteriorId::Office
            && element.id == "stone-fireplace-1-01"
        {
            InteractableKind::Hearth
        } else if map.interior_id == interior::InteriorId::Office && element.id == "old-desk-01" {
            InteractableKind::Desk
        } else {
            InteractableKind::InteriorRepairable
        };
        commands.entity(entity).insert((
            Interactable {
                kind,
                consumed: kind == InteractableKind::InteriorRepairable && state == "repaired",
            },
            MutableSceneElement {
                scene_id: map.id().to_owned(),
                id: element.id.clone(),
                state: state.to_owned(),
            },
        ));
        if kind == InteractableKind::Hearth {
            commands.entity(entity).insert(StoryHotspot {
                beat: StoryBeat::OfficeHearth,
                radius: 104.0,
            });
        }
        if game_audio::is_creaking_floorboard(element) {
            commands
                .entity(entity)
                .insert(game_audio::CreakingFloorboard);
        }
        if !matches!(kind, InteractableKind::Hearth | InteractableKind::Desk) {
            commands.entity(entity).insert(TaskTarget {
                action: element.task.action,
                requirements: element.task.requirements_text(),
            });
        }
    }
    for interaction in map.interactions() {
        let previously_discovered = interior_state
            .0
            .get(&format!("{}/{}", map.id(), interaction.id))
            .is_some_and(|state| state == DISCOVERY_FOUND_STATE);
        let kind = match (interaction.kind, interaction.discovery) {
            (interior::SceneInteractionKind::Search, interior::SceneDiscovery::GideonBible) => {
                InteractableKind::BibleNightstand
            }
        };
        let entity = spawn_interactable(
            commands,
            kind,
            interaction.center,
            interaction.size,
            Color::NONE,
        );
        commands.entity(entity).insert((
            interior::InteriorSceneEntity,
            AuthoredInteractionLabel(if previously_discovered {
                "nightstand where the little book rests".to_owned()
            } else {
                interaction.label.clone()
            }),
            Interactable {
                kind,
                // Discovery records knowledge but does not remove the object:
                // the Scribe deliberately leaves the Bible here to revisit.
                consumed: false,
            },
        ));
    }
    let exit = spawn_interactable(
        commands,
        InteractableKind::InteriorExit,
        map.cell_center(map.exits[0]),
        Vec2::splat(30.0),
        Color::NONE,
    );
    commands.entity(exit).insert(interior::InteriorSceneEntity);
}

fn motel_door_routes(motel: &interior::MotelExteriorMap) -> HashMap<String, interior::InteriorId> {
    let mut door_ids = motel
        .mutable_elements()
        .iter()
        .filter(|element| element.kind == "door")
        .map(|element| (element.pixel_x, element.id.clone()))
        .collect::<Vec<_>>();
    door_ids.sort_by(|left, right| left.0.total_cmp(&right.0));
    assert_eq!(
        door_ids.len(),
        interior::InteriorId::MOTEL.len(),
        "the authored motel must have one exterior door per interior"
    );
    door_ids
        .into_iter()
        .zip(interior::InteriorId::MOTEL)
        .map(|((_, id), interior_id)| (id, interior_id))
        .collect()
}

const fn motel_door_is_unlocked(
    destination: MotelDoorDestination,
    motel_access: &MotelAccess,
) -> bool {
    destination.initially_unlocked || motel_access.keys_found
}

#[cfg(target_arch = "wasm32")]
fn load_story(
    mut story: ResMut<Story>,
    mut interior_state: ResMut<InteriorState>,
    mut motel_access: ResMut<MotelAccess>,
    mut progression: ResMut<Progression>,
) {
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return;
    };
    let Some(raw) = storage.get_item("waystation-save-v1").ok().flatten() else {
        return;
    };
    let Ok(save) = serde_json::from_str::<SaveData>(&raw) else {
        return;
    };
    if !matches!(save.version, 1..=6) || save.vignette_index >= vignettes().len() {
        return;
    }
    let migrate_legacy_bible = save.version <= 4 && story_stage_requires_bible(save.stage);
    story.stage = save.stage;
    story.kindling = save.kindling.min(3);
    story.vignette_index = save.vignette_index;
    story.dialogue_line = save.dialogue_line;
    story.result = save.result;
    story.card = save.card;
    interior_state.0 = save.interior_states;
    if migrate_legacy_bible {
        interior_state
            .0
            .entry(BIBLE_STATE_KEY.to_owned())
            .or_insert_with(|| DISCOVERY_FOUND_STATE.to_owned());
    }
    motel_access.keys_found = save.motel_keys_found;
    *progression = save.progression;
    story.notice = Some("The old trail returns to memory.".to_owned());
}

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
const fn story_stage_requires_bible(stage: StoryStage) -> bool {
    matches!(
        stage,
        StoryStage::FindPlank
            | StoryStage::RestoreDesk
            | StoryStage::Night
            | StoryStage::MeetTraveler
            | StoryStage::Dialogue
            | StoryStage::Interpreting
            | StoryStage::ChoosePaper
            | StoryStage::ChooseIllustration
            | StoryStage::ChooseBorder
            | StoryStage::FinishedCard
            | StoryStage::Epilogue
    )
}

fn bible_found(interior_state: &InteriorState) -> bool {
    interior_state
        .0
        .get(BIBLE_STATE_KEY)
        .is_some_and(|state| state == DISCOVERY_FOUND_STATE)
}

fn record_bible_discovery(story: &mut Story, interior_state: &mut InteriorState) {
    interior_state
        .0
        .insert(BIBLE_STATE_KEY.to_owned(), DISCOVERY_FOUND_STATE.to_owned());
    if story.stage == StoryStage::FindBible {
        story.stage = StoryStage::FindPlank;
    }
}

fn story_beat_seen(interior_state: &InteriorState, beat: StoryBeat) -> bool {
    interior_state
        .0
        .get(beat.state_key())
        .is_some_and(|state| state == STORY_SEEN_STATE)
}

fn present_story_beat(
    popup: &mut NarrativePopup,
    interior_state: &mut InteriorState,
    beat: StoryBeat,
) -> bool {
    if story_beat_seen(interior_state, beat) {
        return false;
    }
    interior_state
        .0
        .insert(beat.state_key().to_owned(), STORY_SEEN_STATE.to_owned());
    popup.present(NarrativeCard::Thought(beat));
    true
}

fn reconcile_office_realization(popup: &mut NarrativePopup, interior_state: &mut InteriorState) {
    let observations = [
        StoryBeat::OfficeThreshold,
        StoryBeat::OfficeHearth,
        StoryBeat::OfficeLedger,
    ];
    if observations
        .into_iter()
        .all(|beat| story_beat_seen(interior_state, beat))
    {
        present_story_beat(popup, interior_state, StoryBeat::OfficeWelcome);
    }
}

#[cfg(not(target_arch = "wasm32"))]
const fn load_story() {}

#[cfg(target_arch = "wasm32")]
fn save_story(
    story: Res<Story>,
    interior_state: Res<InteriorState>,
    motel_access: Res<MotelAccess>,
    progression: Res<Progression>,
) {
    if !story.is_changed()
        && !interior_state.is_changed()
        && !motel_access.is_changed()
        && !progression.is_changed()
    {
        return;
    }
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return;
    };
    if let Ok(raw) = serde_json::to_string(&SaveData::capture(
        &story,
        &interior_state,
        &motel_access,
        &progression,
    )) {
        let _ = storage.set_item("waystation-save-v1", &raw);
    }
}

#[cfg(not(target_arch = "wasm32"))]
const fn save_story() {}

fn spawn_interactable(
    commands: &mut Commands,
    kind: InteractableKind,
    position: Vec2,
    size: Vec2,
    color: Color,
) -> Entity {
    spawn_interactable_sprite(commands, kind, position, Sprite::from_color(color, size))
}

fn nearest_valid_position(desired: Vec2, mut is_valid: impl FnMut(Vec2) -> bool) -> Option<Vec2> {
    if is_valid(desired) {
        return Some(desired);
    }
    for ring in 1..=DROP_SEARCH_RINGS {
        for x in -ring..=ring {
            for y in [-ring, ring] {
                let candidate = desired + Vec2::new(f32::from(x), f32::from(y)) * DROP_SEARCH_STEP;
                if is_valid(candidate) {
                    return Some(candidate);
                }
            }
        }
        for y in (-ring + 1)..ring {
            for x in [-ring, ring] {
                let candidate = desired + Vec2::new(f32::from(x), f32::from(y)) * DROP_SEARCH_STEP;
                if is_valid(candidate) {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn tree_ground_rect(position: Vec2, size: f32) -> ExteriorRect {
    ExteriorRect::new(
        position + Vec2::new(0.0, -size * 0.34),
        Vec2::new(size * 0.28, size * 0.16),
    )
}

fn tree_trunk_rect(position: Vec2, size: f32) -> ExteriorRect {
    let ground = tree_ground_rect(position, size);
    ExteriorRect::new(
        ground.center,
        ground.size + Vec2::splat(TREE_PLAYER_CLEARANCE * 2.0),
    )
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn tree_image_pixel_at_world_point(
    world_point: Vec2,
    tree_position: Vec2,
    tree_size: f32,
    image_size: UVec2,
) -> Option<UVec2> {
    let local = (world_point - tree_position) / tree_size;
    let image_position = Vec2::new(local.x + 0.5, 0.5 - local.y);
    if image_position.x < 0.0
        || image_position.x >= 1.0
        || image_position.y < 0.0
        || image_position.y >= 1.0
    {
        return None;
    }
    Some(UVec2::new(
        (image_position.x * image_size.x as f32).floor() as u32,
        (image_position.y * image_size.y as f32).floor() as u32,
    ))
}

fn tree_image_is_opaque_at_world_point(
    image: &Image,
    tree_position: Vec2,
    tree_size: f32,
    world_point: Vec2,
) -> bool {
    tree_image_pixel_at_world_point(
        world_point,
        tree_position,
        tree_size,
        UVec2::new(image.width(), image.height()),
    )
    .and_then(|pixel| image.get_color_at(pixel.x, pixel.y).ok())
    .is_some_and(|color| color.to_srgba().alpha >= TREE_OPAQUE_ALPHA)
}

fn player_is_fully_covered_by_tree_alpha(
    player_position: Vec2,
    mut is_tree_opaque: impl FnMut(Vec2) -> bool,
) -> bool {
    let mut opaque_samples = 0;
    for row in 0..TREE_OCCLUSION_SAMPLE_ROWS {
        for column in 0..TREE_OCCLUSION_SAMPLE_COLUMNS {
            let offset = Vec2::new(
                f32::from(column).mul_add(TREE_OCCLUSION_SAMPLE_SPACING, -14.0),
                f32::from(row).mul_add(TREE_OCCLUSION_SAMPLE_SPACING, -26.0),
            );
            opaque_samples += usize::from(is_tree_opaque(player_position + offset));
        }
    }
    let sample_count = usize::from(TREE_OCCLUSION_SAMPLE_COLUMNS * TREE_OCCLUSION_SAMPLE_ROWS);
    opaque_samples * 100 >= sample_count * TREE_OCCLUSION_REQUIRED_PERCENT
}

fn dense_tree_occludes_player(
    player_position: Vec2,
    tree_position: Vec2,
    tree_size: f32,
    tree_ground_y: f32,
    tree_image: &Image,
) -> bool {
    let player_ground_y = player_position.y + PLAYER_GROUND_OFFSET_Y;
    let player_art = ExteriorRect::new(
        player_position,
        Vec2::new(SCRIBE_FRAME_SIZE_F32 / 2.0, SCRIBE_FRAME_SIZE_F32 * 0.875),
    );
    player_ground_y > tree_ground_y
        && ExteriorRect::new(tree_position, Vec2::splat(tree_size)).overlaps(player_art)
        && player_is_fully_covered_by_tree_alpha(player_position, |point| {
            tree_image_is_opaque_at_world_point(tree_image, tree_position, tree_size, point)
        })
}

fn resolve_tree_position(
    grid: &terrain::WorldGrid,
    motel: &interior::MotelExteriorMap,
    tool_shed: &interior::ToolShedExteriorMap,
    desired: Vec2,
    size: f32,
) -> Vec2 {
    nearest_valid_position(desired, |candidate| {
        let ground = tree_ground_rect(candidate, size);
        let trunk = tree_trunk_rect(candidate, size);
        grid.supports_land_footprint(ground.center, ground.size)
            && motel.is_area_walkable(trunk.center, trunk.size)
            && tool_shed.is_area_walkable(trunk.center, trunk.size)
    })
    .expect("the generated exterior needs enough land for every tree")
}

fn safe_pickup_position(
    grid: &terrain::WorldGrid,
    motel: &interior::MotelExteriorMap,
    tool_shed: &interior::ToolShedExteriorMap,
    obstacles: &ExteriorObstacles,
    reserved: &[ExteriorRect],
    desired: Vec2,
    size: Vec2,
) -> Vec2 {
    nearest_valid_position(desired, |candidate| {
        let bounds = ExteriorRect::new(candidate, size);
        grid.supports_land_footprint(candidate, size)
            && motel.is_area_walkable(candidate, size)
            && tool_shed.is_area_walkable(candidate, size)
            && obstacles.prop_is_clear(bounds)
            && reserved.iter().all(|occupied| !occupied.overlaps(bounds))
    })
    .expect("the generated exterior needs enough clear land for every pickup")
}

fn spawn_interactable_sprite(
    commands: &mut Commands,
    kind: InteractableKind,
    position: Vec2,
    sprite: Sprite,
) -> Entity {
    commands
        .spawn((
            sprite,
            Transform::from_xyz(position.x, position.y, 1.0),
            Interactable {
                kind,
                consumed: false,
            },
        ))
        .id()
}

fn spawn_world_pickup(
    commands: &mut Commands,
    progression: &Progression,
    id: String,
    kind: InteractableKind,
    position: Vec2,
    sprite: Sprite,
    reward: PickupReward,
) -> Entity {
    let collected = progression.pickup_collected(&id);
    let entity = spawn_interactable_sprite(commands, kind, position, sprite);
    commands.entity(entity).insert((
        WorldPickup { id, reward },
        if collected {
            Visibility::Hidden
        } else {
            Visibility::Visible
        },
    ));
    if collected {
        commands.entity(entity).insert(Interactable {
            kind,
            consumed: true,
        });
    }
    entity
}

#[allow(clippy::too_many_arguments)]
fn spawn_safe_world_pickup(
    commands: &mut Commands,
    progression: &Progression,
    grid: &terrain::WorldGrid,
    motel: &interior::MotelExteriorMap,
    tool_shed: &interior::ToolShedExteriorMap,
    obstacles: &ExteriorObstacles,
    reserved: &mut Vec<ExteriorRect>,
    id: String,
    kind: InteractableKind,
    desired_position: Vec2,
    size: Vec2,
    sprite: Sprite,
    reward: PickupReward,
) -> Entity {
    let position = safe_pickup_position(
        grid,
        motel,
        tool_shed,
        obstacles,
        reserved,
        desired_position,
        size,
    );
    reserved.push(ExteriorRect::new(position, size));
    spawn_world_pickup(commands, progression, id, kind, position, sprite, reward)
}

/// The blocking part of a station is the lower half of its art, so the Scribe
/// can stand close enough to work without walking through the piece itself.
fn station_footprint(position: Vec2, size: Vec2) -> ExteriorRect {
    ExteriorRect::new(position - Vec2::new(0.0, size.y / 4.0), size / 2.0)
}

/// A station the Scribe returns to rather than consumes. It stands on the
/// ground like a tree, so it blocks movement and keeps loose props off itself.
fn spawn_worked_station<'a>(
    commands: &'a mut Commands,
    image: Handle<Image>,
    kind: InteractableKind,
    task: progression::TaskSpec,
    position: Vec2,
    size: Vec2,
    obstacles: &mut ExteriorObstacles,
) -> bevy::ecs::system::EntityCommands<'a> {
    let ground_offset_y = -size.y / 2.0;
    obstacles
        .solid_footprints
        .push(station_footprint(position, size));
    obstacles
        .prop_exclusions
        .push(ExteriorRect::new(position, size));
    commands.spawn((
        Sprite {
            image,
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(
            position.x,
            position.y,
            exterior_depth(position.y + ground_offset_y),
        ),
        ExteriorYSort {
            ground_offset_y,
            depth_bias: 0.0,
        },
        Interactable {
            kind,
            consumed: false,
        },
        TaskTarget {
            action: task.action,
            requirements: task.requirements_text(),
        },
    ))
}

fn spawn_portable_tool_entity(
    commands: &mut Commands,
    asset_server: &AssetServer,
    definition: &PortableToolDefinition,
) -> Entity {
    commands
        .spawn((
            Sprite {
                image: asset_server.load(definition.image_path.clone()),
                custom_size: Some(definition.size),
                flip_x: definition.flip_x,
                flip_y: definition.flip_y,
                ..default()
            },
            Transform::from_xyz(
                definition.home_position.x,
                definition.home_position.y,
                portable_tool_interior_depth(&definition.layer),
            ),
            Visibility::Hidden,
            Interactable {
                kind: InteractableKind::Tool,
                consumed: true,
            },
            AuthoredInteractionLabel(definition.label.clone()),
            PortableToolEntity {
                id: definition.id.clone(),
            },
        ))
        .id()
}

fn portable_tool_interior_depth(layer: &str) -> f32 {
    match layer {
        "floor" => -9.75,
        "wall" => -7.75,
        "object" => -2.75,
        "overlay" => 4.25,
        _ => panic!("unsupported portable-tool layer: {layer}"),
    }
}

fn active_scene_id(location: WorldLocation, interior: &interior::InteriorMap) -> &str {
    if location == WorldLocation::Exterior {
        "exterior"
    } else {
        interior.id()
    }
}

#[allow(clippy::type_complexity)]
fn sync_portable_tool_entities(
    location: Res<WorldLocation>,
    interior: Res<interior::InteriorMap>,
    progression: Res<Progression>,
    catalog: Res<PortableToolCatalog>,
    mut entities: Query<(
        &PortableToolEntity,
        &mut Transform,
        &mut Visibility,
        &mut Interactable,
    )>,
) {
    let scene_id = active_scene_id(*location, &interior);
    for (portable, mut transform, mut visibility, mut interactable) in &mut entities {
        let Some(definition) = catalog.0.iter().find(|item| item.id == portable.id) else {
            continue;
        };
        let Some(record) = progression.tool_record(&portable.id) else {
            continue;
        };
        let position = match &record.location {
            ToolLocation::Home if definition.home_scene == scene_id => {
                Some(definition.home_position)
            }
            ToolLocation::Dropped {
                scene_id: dropped_scene,
                x,
                y,
            } if dropped_scene == scene_id => Some(IVec2::new(*x, *y).as_vec2()),
            _ => None,
        };
        let Some(position) = position else {
            *visibility = Visibility::Hidden;
            interactable.consumed = true;
            continue;
        };
        transform.translation.x = position.x;
        transform.translation.y = position.y;
        transform.translation.z = if *location == WorldLocation::Exterior {
            exterior_depth(position.y - definition.size.y / 2.0)
        } else {
            portable_tool_interior_depth(&definition.layer)
        };
        *visibility = Visibility::Visible;
        interactable.consumed = false;
    }
}

fn handle_tool_hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    popup: Res<NarrativePopup>,
    environment: ToolDropEnvironment,
    mut progression: ResMut<Progression>,
    mut story: ResMut<Story>,
    player: Query<(&Transform, &PlayerAnimation), With<Player>>,
) {
    if popup.is_open() {
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        story.notice = progression.cycle_equipped_tool().map(|tool| {
            format!(
                "You shift the {} where it is easiest to reach.",
                tool.label()
            )
        });
    }
    if !keys.just_pressed(KeyCode::KeyQ) {
        return;
    }
    let Ok((transform, animation)) = player.single() else {
        return;
    };
    let Some(equipped_id) = progression.equipped_tool_id() else {
        story.notice = Some("You are not carrying a portable tool to put down.".to_owned());
        return;
    };
    let Some(definition) = environment
        .catalog
        .0
        .iter()
        .find(|item| item.id == equipped_id)
    else {
        return;
    };
    let returning_home = *environment.location == WorldLocation::Interior
        && environment.interior.interior_id == interior::InteriorId::ToolShed
        && definition.home_scene == environment.interior.id();
    let tool = if returning_home {
        progression.return_equipped_tool()
    } else {
        let offset = match animation.facing {
            Facing::Up => Vec2::new(0.0, 30.0),
            Facing::Left => Vec2::new(-30.0, -18.0),
            Facing::Down => Vec2::new(0.0, -42.0),
            Facing::Right => Vec2::new(30.0, -18.0),
        };
        let desired = transform.translation.truncate() + offset;
        let position = nearest_valid_position(desired, |candidate| {
            if *environment.location == WorldLocation::Exterior {
                let bounds = ExteriorRect::new(candidate, definition.size);
                environment
                    .terrain
                    .supports_land_footprint(candidate, definition.size)
                    && environment
                        .motel
                        .is_area_walkable(candidate, definition.size)
                    && environment
                        .tool_shed
                        .is_area_walkable(candidate, definition.size)
                    && environment.obstacles.prop_is_clear(bounds)
            } else {
                environment
                    .interior
                    .is_area_walkable(candidate, definition.size)
            }
        })
        .unwrap_or_else(|| transform.translation.truncate())
        .round();
        let position = position.as_ivec2();
        progression.drop_equipped_tool(
            active_scene_id(*environment.location, &environment.interior),
            (position.x, position.y),
        )
    };
    story.notice = tool.map(|tool| {
        if returning_home {
            format!(
                "You return the {} to its place in the tool shed.",
                tool.label()
            )
        } else {
            format!("You set down the {}. It will remain here.", tool.label())
        }
    });
}

fn load_ui_fonts(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(UiFonts {
        roman: asset_server.load(ROMAN_FONT_PATH),
        emoji: asset_server.load(EMOJI_FONT_PATH),
    });
}

#[allow(clippy::too_many_lines)]
fn setup_ui(mut commands: Commands, asset_server: Res<AssetServer>, fonts: Res<UiFonts>) {
    terrain::spawn_debug_legend(&mut commands);
    let parchment = BackgroundColor(Color::srgba(0.72, 0.63, 0.45, 0.95));
    let parchment_edge = BorderColor::all(Color::srgb(0.28, 0.20, 0.12));
    let ink = TextColor(Color::srgb(0.16, 0.11, 0.07));
    let quiet_ink = TextColor(Color::srgb(0.25, 0.18, 0.11));
    commands
        .spawn((
            Text::new("📜  "),
            fonts.emoji(10.0),
            ink,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(18.0),
                top: Val::Px(16.0),
                width: Val::Px(180.0),
                max_width: Val::Percent(44.0),
                padding: UiRect::axes(Val::Px(7.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            parchment,
            parchment_edge,
            Outline::new(
                Val::Px(1.0),
                Val::Px(1.0),
                Color::srgba(0.06, 0.04, 0.02, 0.55),
            ),
        ))
        .with_child((TextSpan::new(""), fonts.roman(10.0), ink, StatusText));
    commands.spawn((
        Text::new(""),
        fonts.roman(8.0),
        quiet_ink,
        TextLayout::new_with_justify(Justify::Right),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(18.0),
            top: Val::Px(16.0),
            width: Val::Px(120.0),
            max_width: Val::Percent(30.0),
            padding: UiRect::axes(Val::Px(7.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        parchment,
        parchment_edge,
        Outline::new(
            Val::Px(1.0),
            Val::Px(1.0),
            Color::srgba(0.06, 0.04, 0.02, 0.55),
        ),
        ProgressText,
    ));
    commands
        .spawn((
            Text::new("☞  "),
            fonts.emoji(10.0),
            ink,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(19.0),
                bottom: Val::Px(18.0),
                width: Val::Percent(62.0),
                padding: UiRect::axes(Val::Px(7.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            parchment,
            parchment_edge,
            Outline::new(
                Val::Px(1.0),
                Val::Px(1.0),
                Color::srgba(0.06, 0.04, 0.02, 0.55),
            ),
        ))
        .with_child((TextSpan::new(""), fonts.roman(10.0), ink, PromptText));

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(14.0),
                right: Val::Percent(14.0),
                top: Val::Percent(15.0),
                bottom: Val::Percent(15.0),
                padding: UiRect::all(Val::Px(30.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(22.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.07, 0.06, 0.96)),
            BorderColor::all(Color::srgb(0.58, 0.48, 0.28)),
            Outline::new(Val::Px(3.0), Val::Px(0.0), Color::srgb(0.58, 0.48, 0.28)),
            Visibility::Hidden,
            OverlayRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                fonts.roman(30.0),
                TextColor(Color::srgb(0.95, 0.79, 0.39)),
                OverlayTitle,
            ));
            parent.spawn((
                ImageNode::new(asset_server.load("card/illustration_1_1.png")),
                Node {
                    width: Val::Px(288.0),
                    height: Val::Px(192.0),
                    ..default()
                },
                Visibility::Hidden,
                CardArt,
            ));
            parent.spawn((
                Text::new(""),
                fonts.roman(20.0),
                TextColor(Color::srgb(0.93, 0.90, 0.80)),
                Node {
                    max_width: Val::Px(680.0),
                    ..default()
                },
                OverlayBody,
            ));
            parent.spawn((
                Text::new(""),
                fonts.roman(13.0),
                TextColor(Color::srgb(0.62, 0.66, 0.61)),
                ProvenanceText,
            ));
        });

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.03, 0.02, 0.56)),
            ZIndex(200),
            Visibility::Hidden,
            NarrativePopupRoot,
        ))
        .with_children(|backdrop| {
            backdrop
                .spawn((
                    Node {
                        width: Val::Px(260.0),
                        max_width: Val::Percent(68.0),
                        padding: UiRect::all(Val::Px(12.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(8.0),
                        ..default()
                    },
                    parchment,
                    parchment_edge,
                    Outline::new(
                        Val::Px(2.0),
                        Val::Px(2.0),
                        Color::srgba(0.04, 0.025, 0.01, 0.72),
                    ),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new(""),
                        fonts.roman(15.0),
                        ink,
                        TextLayout::new_with_justify(Justify::Center),
                        NarrativePopupTitle,
                    ));
                    panel.spawn((
                        ImageNode::new(asset_server.load(BIBLE_ICON_PATH)),
                        Node {
                            display: Display::None,
                            width: Val::Px(34.0),
                            height: Val::Px(34.0),
                            ..default()
                        },
                        NarrativePopupArt,
                    ));
                    panel.spawn((
                        Text::new(""),
                        fonts.roman(10.5),
                        ink,
                        TextLayout::new_with_justify(Justify::Center),
                        Node {
                            max_width: Val::Px(232.0),
                            ..default()
                        },
                        NarrativePopupBody,
                    ));
                    panel.spawn((
                        Text::new("E / SPACE — continue"),
                        fonts.roman(8.0),
                        quiet_ink,
                        TextLayout::new_with_justify(Justify::Center),
                        NarrativePopupDismiss,
                    ));
                });
        });
}

fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    story: Res<Story>,
    popup: Res<NarrativePopup>,
    mut environment: MovementEnvironment,
    mut player: Query<(&mut Transform, &mut PlayerAnimation), With<Player>>,
) {
    environment.doorway_attempt.0 = None;
    if popup.is_open()
        || matches!(
            story.stage,
            StoryStage::Night
                | StoryStage::Dialogue
                | StoryStage::Interpreting
                | StoryStage::ChoosePaper
                | StoryStage::ChooseIllustration
                | StoryStage::ChooseBorder
                | StoryStage::FinishedCard
                | StoryStage::Epilogue
        )
    {
        return;
    }
    let Ok((mut transform, mut animation)) = player.single_mut() else {
        return;
    };
    if animation.active_tool.is_some() {
        return;
    }
    let mut direction = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }
    if direction != Vec2::ZERO {
        animation.facing = if direction.x.abs() > direction.y.abs() {
            if direction.x < 0.0 {
                Facing::Left
            } else {
                Facing::Right
            }
        } else if direction.y > 0.0 {
            Facing::Up
        } else {
            Facing::Down
        };
        let delta = direction.normalize() * PLAYER_SPEED * time.delta_secs();
        let mut next = transform.translation.truncate();
        if *environment.location == WorldLocation::Exterior {
            next = move_player_outside(next, delta, &mut environment);
        } else {
            let next_x = Vec2::new(next.x + delta.x, next.y);
            if player_area_is_walkable_interior(next_x, &environment.interior) {
                next.x = next_x.x;
            }
            let next_y = Vec2::new(next.x, next.y + delta.y);
            if player_area_is_walkable_interior(next_y, &environment.interior) {
                next.y = next_y.y;
            }
        }
        transform.translation.x = next.x;
        transform.translation.y = next.y;
    }
}

fn move_player_outside(
    mut next: Vec2,
    delta: Vec2,
    environment: &mut MovementEnvironment<'_, '_>,
) -> Vec2 {
    let next_x = Vec2::new(
        (next.x + delta.x).clamp(-MAP_HALF_WIDTH, MAP_HALF_WIDTH),
        next.y,
    );
    if exterior_position_is_walkable(
        next_x,
        &environment.terrain,
        &environment.motel,
        &environment.tool_shed,
        &environment.obstacles,
    ) {
        next.x = next_x.x;
    }
    let next_y = Vec2::new(
        next.x,
        (next.y + delta.y).clamp(-MAP_HALF_HEIGHT, MAP_HALF_HEIGHT),
    );
    let head_probe = next_y + Vec2::Y * DOOR_HEAD_PROBE_OFFSET;
    let head_hits_building = !environment.motel.is_walkable(head_probe)
        || !environment.tool_shed.is_walkable(head_probe);
    let doorway = if delta.y > 0.0 && head_hits_building {
        environment
            .doors
            .iter()
            .filter(|(transform, sprite, _)| {
                player_inside_doorway(
                    next_y,
                    transform.translation.truncate(),
                    sprite.custom_size.unwrap_or(Vec2::splat(64.0)),
                )
            })
            .min_by(|(left, _, _), (right, _, _)| {
                (next_y.x - left.translation.x)
                    .abs()
                    .total_cmp(&(next_y.x - right.translation.x).abs())
            })
            .map(|(_, _, destination)| *destination)
    } else {
        None
    };
    if let Some(destination) = doorway {
        environment.doorway_attempt.0 = Some(destination);
    } else if exterior_position_is_walkable(
        next_y,
        &environment.terrain,
        &environment.motel,
        &environment.tool_shed,
        &environment.obstacles,
    ) {
        next.y = next_y.y;
    }
    next
}

fn exterior_position_is_walkable(
    position: Vec2,
    grid: &terrain::WorldGrid,
    motel: &interior::MotelExteriorMap,
    tool_shed: &interior::ToolShedExteriorMap,
    obstacles: &ExteriorObstacles,
) -> bool {
    let bounds = player_collision_rect(position);
    grid.supports_land_footprint(bounds.center, bounds.size)
        && motel.is_area_walkable(bounds.center, bounds.size)
        && tool_shed.is_area_walkable(bounds.center, bounds.size)
        && obstacles.player_can_stand(bounds)
}

fn player_area_is_walkable_interior(position: Vec2, interior: &interior::InteriorMap) -> bool {
    let bounds = player_collision_rect(position);
    interior.is_area_walkable(bounds.center, bounds.size)
}

fn player_collision_rect(position: Vec2) -> ExteriorRect {
    ExteriorRect::new(position + PLAYER_COLLISION_OFFSET, PLAYER_COLLISION_SIZE)
}

fn exterior_depth(ground_y: f32) -> f32 {
    ground_y.mul_add(-EXTERIOR_DEPTH_PER_Y, EXTERIOR_DEPTH_BASE)
}

fn authored_layer_index(layer: &str) -> usize {
    match layer {
        "floor" => 0,
        "wall" => 1,
        "object" => 2,
        "overlay" => 3,
        _ => panic!("unsupported authored scene layer: {layer}"),
    }
}

#[allow(clippy::cast_precision_loss)]
fn building_layer_depth_bias(layer_index: usize, mutable: bool) -> f32 {
    let sublayer = layer_index * 2 + usize::from(mutable) + 1;
    sublayer as f32 * BUILDING_LAYER_DEPTH_STEP
}

fn building_occlusion_crown_depth(building_ground_y: f32) -> f32 {
    9.0f32.mul_add(BUILDING_LAYER_DEPTH_STEP, exterior_depth(building_ground_y))
}

/// Covering scenery keeps a narrow Y-sorted band of its own so two overlapping
/// occluders in front of the player still stack southernmost-first.
fn interior_occluder_cover_depth(ground_y: f32) -> f32 {
    ground_y.mul_add(
        -INTERIOR_OCCLUDER_DEPTH_PER_Y,
        INTERIOR_OCCLUDER_COVER_DEPTH,
    )
}

fn interior_occluder_covers_player(player_ground_y: f32, occluder_ground_y: f32) -> bool {
    player_ground_y > occluder_ground_y
}

fn scribe_occlusion_crown_offset_y() -> f32 {
    (SCRIBE_FRAME_SIZE_F32 - SCRIBE_OCCLUSION_CROWN_HEIGHT) / 2.0
}

fn update_exterior_depth(
    location: Res<WorldLocation>,
    mut sorted: Query<(&mut Transform, &ExteriorYSort), Without<ExteriorFixedDepth>>,
    mut fixed: Query<(&mut Transform, &ExteriorFixedDepth), Without<ExteriorYSort>>,
) {
    if *location != WorldLocation::Exterior {
        return;
    }
    for (mut transform, sorting) in &mut sorted {
        transform.translation.z =
            exterior_depth(transform.translation.y + sorting.ground_offset_y) + sorting.depth_bias;
    }
    for (mut transform, sorting) in &mut fixed {
        transform.translation.z = exterior_depth(sorting.ground_y) + sorting.depth_bias;
    }
}

/// Interiors have no Y-sorted ground plane, so authored walk-behind scenery
/// swaps between its layer depth and a band above the Scribe as they cross its
/// floor line.
fn update_interior_occlusion(
    location: Res<WorldLocation>,
    mut player: Query<&mut Transform, With<Player>>,
    mut occluders: Query<(&mut Transform, &interior::InteriorOccluder), Without<Player>>,
) {
    if *location != WorldLocation::Interior {
        return;
    }
    let Ok(mut player_transform) = player.single_mut() else {
        return;
    };
    player_transform.translation.z = INTERIOR_PLAYER_DEPTH;
    let player_ground_y = player_transform.translation.y + PLAYER_GROUND_OFFSET_Y;
    for (mut transform, occluder) in &mut occluders {
        transform.translation.z =
            if interior_occluder_covers_player(player_ground_y, occluder.ground_y) {
                interior_occluder_cover_depth(occluder.ground_y)
            } else {
                occluder.resting_depth
            };
    }
}

#[allow(clippy::type_complexity)]
fn update_player_tree_occlusion(
    location: Res<WorldLocation>,
    images: Res<Assets<Image>>,
    mut player: Query<(&Transform, &mut Visibility), With<Player>>,
    trees: Query<(&Transform, &Sprite, &ExteriorYSort), (With<DenseTreeOccluder>, Without<Player>)>,
) {
    let Ok((player_transform, mut visibility)) = player.single_mut() else {
        return;
    };
    if *location != WorldLocation::Exterior {
        *visibility = Visibility::Visible;
        return;
    }
    let player_position = player_transform.translation.truncate();
    let occluded = trees.iter().any(|(tree_transform, tree_sprite, sorting)| {
        let tree_position = tree_transform.translation.truncate();
        let tree_size = tree_sprite.custom_size.unwrap_or(Vec2::ONE).x;
        images.get(&tree_sprite.image).is_some_and(|tree_image| {
            dense_tree_occludes_player(
                player_position,
                tree_position,
                tree_size,
                tree_position.y + sorting.ground_offset_y,
                tree_image,
            )
        })
    });
    *visibility = if occluded {
        Visibility::Hidden
    } else {
        Visibility::Visible
    };
}

#[allow(clippy::type_complexity)]
fn sync_player_occlusion_crown(
    location: Res<WorldLocation>,
    motel: Res<interior::MotelExteriorMap>,
    tool_shed: Res<interior::ToolShedExteriorMap>,
    player: Query<(&Transform, &Sprite, &Visibility, &PlayerAnimation), With<Player>>,
    mut crown: Query<
        (&mut Transform, &mut Sprite, &mut Visibility),
        (
            With<PlayerOcclusionCrown>,
            Without<Player>,
            Without<BuildingCrownOccluder>,
        ),
    >,
    full_occluders: Query<
        (&Transform, &Sprite),
        (With<BuildingCrownOccluder>, Without<PlayerOcclusionCrown>),
    >,
) {
    let (
        Ok((player_transform, player_sprite, player_visibility, player_animation)),
        Ok((mut transform, mut sprite, mut visibility)),
    ) = (player.single(), crown.single_mut())
    else {
        return;
    };
    let player_ground = player_transform.translation.truncate() + Vec2::Y * PLAYER_GROUND_OFFSET_Y;
    let occluding_building_ground_y = if motel.occludes_ground_point(player_ground) {
        Some(motel.depth_ground_y())
    } else if tool_shed.occludes_ground_point(player_ground) {
        Some(tool_shed.depth_ground_y())
    } else {
        None
    };
    if *location != WorldLocation::Exterior
        || *player_visibility == Visibility::Hidden
        || player_animation.active_tool.is_some()
        || occluding_building_ground_y.is_none()
    {
        *visibility = Visibility::Hidden;
        return;
    }

    let Some(player_atlas) = player_sprite.texture_atlas.as_ref() else {
        *visibility = Visibility::Hidden;
        return;
    };
    if let Some(crown_atlas) = sprite.texture_atlas.as_mut() {
        crown_atlas.index = player_atlas.index;
    }
    let crown_position = Vec2::new(
        player_transform.translation.x,
        player_transform.translation.y + scribe_occlusion_crown_offset_y(),
    );
    let crown_bounds = ExteriorRect::new(
        crown_position,
        Vec2::new(SCRIBE_OCCLUSION_CROWN_WIDTH, SCRIBE_OCCLUSION_CROWN_HEIGHT),
    );
    let hidden_by_tall_structure = motel
        .fully_occludes_crown(crown_bounds.center, crown_bounds.size)
        || tool_shed.fully_occludes_crown(crown_bounds.center, crown_bounds.size)
        || full_occluders.iter().any(|(occluder, sprite)| {
            crown_bounds.overlaps(ExteriorRect::new(
                occluder.translation.truncate(),
                sprite.custom_size.unwrap_or(Vec2::ONE),
            ))
        });
    if hidden_by_tall_structure {
        *visibility = Visibility::Hidden;
        return;
    }
    transform.translation.x = crown_position.x;
    transform.translation.y = crown_position.y;
    transform.translation.z = building_occlusion_crown_depth(
        occluding_building_ground_y.expect("an occluding building was selected"),
    );
    *visibility = Visibility::Visible;
}

fn player_inside_doorway(player_position: Vec2, door_position: Vec2, door_size: Vec2) -> bool {
    let half_size = door_size / 2.0;
    (player_position.x - door_position.x).abs() <= half_size.x
        && player_position.y >= door_position.y - half_size.y
        && player_position.y <= door_position.y + half_size.y
}

fn latch_locked_door_bump(
    latched_door: &mut Option<interior::InteriorId>,
    door: interior::InteriorId,
) -> bool {
    if *latched_door == Some(door) {
        false
    } else {
        *latched_door = Some(door);
        true
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_automatic_doorways(
    mut commands: Commands,
    mut story: ResMut<Story>,
    mut location: ResMut<WorldLocation>,
    interior: Res<interior::InteriorMap>,
    motel_access: Res<MotelAccess>,
    mut narrative: NarrativeResources,
    keys: Res<ButtonInput<KeyCode>>,
    asset_server: Res<AssetServer>,
    mut exterior_return: ResMut<ExteriorReturn>,
    mut doorway_attempt: ResMut<DoorwayAttempt>,
    mut door_bump_latch: ResMut<DoorBumpLatch>,
    mut player: Query<&mut Transform, With<Player>>,
    interior_entities: Query<Entity, With<interior::InteriorSceneEntity>>,
) {
    let Ok(mut player_transform) = player.single_mut() else {
        return;
    };
    let player_position = player_transform.translation.truncate();
    if *location == WorldLocation::Exterior {
        let pushing_up = keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp);
        if !pushing_up {
            door_bump_latch.0 = None;
        }
        let Some(destination) = doorway_attempt.0.take() else {
            return;
        };
        if !motel_door_is_unlocked(destination, &motel_access) {
            if latch_locked_door_bump(&mut door_bump_latch.0, destination.interior_id) {
                player_transform.translation.y -= LOCKED_DOOR_BUMP_DISTANCE;
                story.notice = Some(format!(
                    "The door to {} is locked. The office may still hold its key.",
                    destination.interior_id.door_label()
                ));
            }
            return;
        }

        door_bump_latch.0 = None;
        for entity in &interior_entities {
            commands.entity(entity).despawn();
        }
        let next_interior = interior::InteriorMap::load(destination.interior_id);
        spawn_interior_scene(
            &mut commands,
            &asset_server,
            &next_interior,
            &narrative.interior_state,
        );
        let position = next_interior.cell_center(next_interior.entry);
        player_transform.translation.x = position.x;
        player_transform.translation.y = position.y;
        exterior_return.0 = destination.doorstep;
        *location = WorldLocation::Interior;
        story.notice = Some(format!(
            "Inside {}, the valley light falls away behind you.",
            next_interior.name()
        ));
        match destination.interior_id {
            interior::InteriorId::Office => {
                present_story_beat(
                    &mut narrative.popup,
                    &mut narrative.interior_state,
                    StoryBeat::OfficeThreshold,
                );
            }
            interior::InteriorId::Room03 => {
                present_story_beat(
                    &mut narrative.popup,
                    &mut narrative.interior_state,
                    StoryBeat::RoomThreePreserved,
                );
            }
            _ => {}
        }
        commands.insert_resource(next_interior);
    } else if interior.is_exit(player_position) {
        player_transform.translation.x = exterior_return.0.x;
        player_transform.translation.y = exterior_return.0.y;
        *location = WorldLocation::Exterior;
        for entity in &interior_entities {
            commands.entity(entity).despawn();
        }
        story.notice = Some("You step back into the valley air.".to_owned());
    }
}

fn animate_player(
    time: Res<Time>,
    art: Res<PlayerArt>,
    mut player: Query<(&Transform, &mut Sprite, &mut PlayerAnimation), With<Player>>,
) {
    let Ok((transform, mut sprite, mut animation)) = player.single_mut() else {
        return;
    };
    let position = transform.translation.truncate();
    if let Some(mut active) = animation.active_tool {
        let (image, layout) = match active.kind {
            ToolWorkAnimation::Hammer => (&art.hammer_image, &art.hammer_layout),
            ToolWorkAnimation::Axe => (&art.axe_image, &art.axe_layout),
        };
        sprite.image = image.clone();
        sprite.texture_atlas = Some(TextureAtlas {
            layout: layout.clone(),
            index: animation.facing.direction_index() * SCRIBE_TOOL_COLUMNS as usize + active.frame,
        });
        animation.timer.tick(time.delta());
        if animation.timer.just_finished() {
            active.frame += 1;
            if active.frame >= SCRIBE_TOOL_COLUMNS as usize {
                animation.active_tool = None;
                animation.frame = 0;
                animation
                    .timer
                    .set_duration(std::time::Duration::from_secs_f32(
                        SCRIBE_WALK_SECONDS_PER_FRAME,
                    ));
                animation.timer.reset();
                sprite.image = art.walk_image.clone();
                sprite.texture_atlas = Some(TextureAtlas {
                    layout: art.walk_layout.clone(),
                    index: animation.facing.walk_row() * SCRIBE_ATLAS_COLUMNS as usize,
                });
            } else {
                animation.active_tool = Some(active);
            }
        }
        animation.last_position = position;
        return;
    }
    let moving = position.distance_squared(animation.last_position) > 0.01;
    if moving {
        animation.timer.tick(time.delta());
        if animation.timer.just_finished() {
            animation.frame = (animation.frame + 1) % SCRIBE_WALK_FRAMES;
        }
    } else {
        animation.frame = 0;
        animation.timer.reset();
    }
    if let Some(atlas) = &mut sprite.texture_atlas {
        atlas.index = animation.facing.walk_row() * SCRIBE_ATLAS_COLUMNS as usize + animation.frame;
    }
    animation.last_position = position;
}

fn start_tool_animation(animation: &mut PlayerAnimation, kind: ToolWorkAnimation) {
    animation.active_tool = Some(ActiveToolAnimation { kind, frame: 0 });
    animation
        .timer
        .set_duration(std::time::Duration::from_secs_f32(
            SCRIBE_TOOL_SECONDS_PER_FRAME,
        ));
    animation.timer.reset();
}

fn start_task_animation(
    task: &progression::TaskSpec,
    commands: &mut Commands,
    asset_server: &AssetServer,
    player_animation: &mut Query<&mut PlayerAnimation, With<Player>>,
) {
    if !task.tools.contains(&ToolId::Hammer) {
        return;
    }
    if let Ok(mut animation) = player_animation.single_mut() {
        start_tool_animation(&mut animation, ToolWorkAnimation::Hammer);
    }
    game_audio::play_hammering(commands, asset_server);
}

#[allow(clippy::type_complexity)]
fn follow_player(
    location: Res<WorldLocation>,
    interior: Res<interior::InteriorMap>,
    player: Query<&Transform, (With<Player>, Without<MainCamera>)>,
    mut camera: Query<(&mut Transform, &mut Projection), (With<MainCamera>, Without<Player>)>,
) {
    let (Ok(player), Ok((mut camera, mut projection))) = (player.single(), camera.single_mut())
    else {
        return;
    };
    let camera_scale = if *location == WorldLocation::Exterior {
        DEVELOPMENT_PRESENTATION_SCALE.recip()
    } else {
        INTERIOR_CAMERA_SCALE
    };
    if let Projection::Orthographic(orthographic) = &mut *projection {
        orthographic.scale = camera_scale;
    }
    let position = if *location == WorldLocation::Exterior {
        Vec2::new(
            player.translation.x.clamp(
                -MAP_HALF_WIDTH + CAMERA_HALF_WIDTH,
                MAP_HALF_WIDTH - CAMERA_HALF_WIDTH,
            ),
            player.translation.y.clamp(
                -MAP_HALF_HEIGHT + CAMERA_HALF_HEIGHT,
                MAP_HALF_HEIGHT - CAMERA_HALF_HEIGHT,
            ),
        )
    } else {
        interior.camera_position(
            player.translation.truncate(),
            Vec2::new(480.0 * camera_scale, 270.0 * camera_scale),
        )
    };
    camera.translation.x = position.x.round();
    camera.translation.y = position.y.round();
}

fn update_nearby_interaction(
    player: Query<&Transform, With<Player>>,
    interactables: Query<(Entity, &Transform, &Interactable, &Visibility), Without<Player>>,
    mut nearby: ResMut<Nearby>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    let mut closest = None;
    let mut best_distance = INTERACT_DISTANCE;
    for (entity, transform, interactable, visibility) in &interactables {
        if interactable.consumed || *visibility == Visibility::Hidden {
            continue;
        }
        if matches!(
            interactable.kind,
            InteractableKind::MotelDoor | InteractableKind::InteriorExit
        ) {
            continue;
        }
        let distance = player
            .translation
            .truncate()
            .distance(transform.translation.truncate());
        if distance < best_distance {
            best_distance = distance;
            closest = Some(entity);
        }
    }
    nearby.0 = closest;
}

fn trigger_story_hotspots(
    location: Res<WorldLocation>,
    player: Query<&Transform, With<Player>>,
    hotspots: Query<(&Transform, &StoryHotspot), Without<Player>>,
    mut popup: ResMut<NarrativePopup>,
    mut interior_state: ResMut<InteriorState>,
) {
    if *location != WorldLocation::Interior || popup.is_open() {
        return;
    }
    let Ok(player) = player.single() else {
        return;
    };
    for (transform, hotspot) in &hotspots {
        if player
            .translation
            .truncate()
            .distance(transform.translation.truncate())
            <= hotspot.radius
            && present_story_beat(&mut popup, &mut interior_state, hotspot.beat)
        {
            reconcile_office_realization(&mut popup, &mut interior_state);
            return;
        }
    }
}

const fn interaction_key_matches(
    kind: InteractableKind,
    stage: StoryStage,
    interact_pressed: bool,
    repair_pressed: bool,
) -> bool {
    match kind {
        InteractableKind::InteriorRepairable
        | InteractableKind::ExteriorRepairable
        | InteractableKind::Tree
        | InteractableKind::Sawbuck
        | InteractableKind::StoneOutcrop => repair_pressed,
        InteractableKind::Tool => interact_pressed || repair_pressed,
        InteractableKind::Desk if matches!(stage, StoryStage::RestoreDesk) => {
            interact_pressed || repair_pressed
        }
        _ => interact_pressed,
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn handle_interaction(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    nearby: Res<Nearby>,
    mut story: ResMut<Story>,
    mut popup: ResMut<NarrativePopup>,
    interior: Res<interior::InteriorMap>,
    motel: Res<interior::MotelExteriorMap>,
    tool_shed: Res<interior::ToolShedExteriorMap>,
    asset_server: Res<AssetServer>,
    mut resources: InteractionResources,
    mut exterior_obstacles: ResMut<ExteriorObstacles>,
    mut queries: InteractionQueries,
) {
    if popup.is_open() {
        return;
    }
    let Some(entity) = nearby.0 else {
        return;
    };
    let Ok(mut target) = queries.interactables.get_mut(entity) else {
        return;
    };
    let interact_pressed = keys.just_pressed(KeyCode::KeyE);
    let repair_pressed = keys.just_pressed(KeyCode::KeyR);
    if !interaction_key_matches(target.kind, story.stage, interact_pressed, repair_pressed) {
        return;
    }
    if let Ok((portable, label)) = queries.portable_tools.get(entity) {
        let Some(record) = resources.progression.tool_record(&portable.id).cloned() else {
            return;
        };
        if repair_pressed {
            if record.condition == ToolCondition::Serviceable {
                story.notice = Some(format!("The {} is already in working order.", label.0));
                return;
            }
            let task = progression::TaskSpec::for_tool_repair(record.tool);
            let outcome = match resources.progression.attempt(&task) {
                Ok(outcome) => outcome,
                Err(reason) => {
                    story.notice = Some(format!(
                        "You cannot repair the {} yet. {reason}\nRequires: {}.",
                        label.0,
                        task.requirements_text()
                    ));
                    return;
                }
            };
            resources
                .progression
                .set_tool_condition(&portable.id, ToolCondition::Serviceable);
            if task.tools.contains(&ToolId::Hammer) {
                if let Ok(mut animation) = queries.player_animation.single_mut() {
                    start_tool_animation(&mut animation, ToolWorkAnimation::Hammer);
                }
                game_audio::play_hammering(&mut commands, &asset_server);
            }
            story.notice = Some(format!(
                "You put the {} back into working order. +{} Upkeep experience.",
                label.0, task.xp
            ));
            if outcome.new_level > outcome.old_level {
                story.notice = Some(format!(
                    "You put the {} back into working order. Upkeep rises to level {}!",
                    label.0, outcome.new_level
                ));
            }
            return;
        }
        match resources.progression.pick_up_tool(&portable.id) {
            Ok(tool) => {
                target.consumed = true;
                commands.entity(entity).insert(Visibility::Hidden);
                story.notice = Some(if record.condition == ToolCondition::Broken {
                    format!(
                        "You take the {}. It is {}, but perhaps it can be repaired. Carried tools: {}/{}.",
                        label.0,
                        record.condition.label(),
                        resources.progression.carried_tool_count(),
                        progression::MAX_CARRIED_TOOLS
                    )
                } else {
                    format!(
                        "You take the {}. It is ready for work. Carried tools: {}/{}.",
                        tool.label(),
                        resources.progression.carried_tool_count(),
                        progression::MAX_CARRIED_TOOLS
                    )
                });
            }
            Err(reason) => story.notice = Some(reason),
        }
        return;
    }
    if let Ok(tree) = queries.choppable_trees.get(entity) {
        let task = progression::TaskSpec::for_tree_chopping();
        let outcome = match resources.progression.attempt(&task) {
            Ok(outcome) => outcome,
            Err(reason) => {
                story.notice = Some(format!(
                    "You cannot cut this tree yet. {reason}\nRequires: {}.",
                    task.requirements_text()
                ));
                return;
            }
        };
        resources.progression.collect_pickup(&tree.id);
        exterior_obstacles
            .solid_footprints
            .retain(|area| !same_exterior_rect(*area, tree.trunk));
        exterior_obstacles
            .prop_exclusions
            .retain(|area| !same_exterior_rect(*area, tree.art));
        if let Ok(mut animation) = queries.player_animation.single_mut() {
            start_tool_animation(&mut animation, ToolWorkAnimation::Axe);
        }
        commands.entity(entity).despawn();
        story.notice = Some(format!(
            "The old axe bites cleanly. You keep two useful logs and gather dry splinters. +{} Upkeep experience{}.",
            task.xp,
            if outcome.new_level > outcome.old_level {
                format!("; Upkeep rises to level {}", outcome.new_level)
            } else {
                String::new()
            }
        ));
        return;
    }
    if target.kind == InteractableKind::Sawbuck {
        let task = progression::TaskSpec::for_milling();
        if let Err(reason) = resources.progression.attempt(&task) {
            story.notice = Some(format!(
                "You cannot mill a plank yet. {reason}\nRequires: {}.",
                task.requirements_text()
            ));
            return;
        }
        if let Ok(mut animation) = queries.player_animation.single_mut() {
            start_tool_animation(&mut animation, ToolWorkAnimation::Axe);
        }
        story.notice = Some(format!(
            "You lay a log across the sawbuck and cut it down to {}. Planks: {}. Logs left: {}.",
            task.yields_text(),
            resources.progression.supply(SupplyId::Plank),
            resources.progression.supply(SupplyId::Log)
        ));
        return;
    }
    if let Ok(outcrop) = queries.stone_outcrops.get(entity) {
        let task = progression::TaskSpec::for_quarrying();
        if let Err(reason) = resources.progression.attempt(&task) {
            story.notice = Some(format!(
                "You cannot work this stone yet. {reason}\nRequires: {}.",
                task.requirements_text()
            ));
            return;
        }
        resources.progression.collect_pickup(&outcrop.id);
        exterior_obstacles
            .solid_footprints
            .retain(|area| !same_exterior_rect(*area, outcrop.footprint));
        exterior_obstacles
            .prop_exclusions
            .retain(|area| !same_exterior_rect(*area, outcrop.art));
        if let Ok(mut animation) = queries.player_animation.single_mut() {
            start_tool_animation(&mut animation, ToolWorkAnimation::Hammer);
        }
        commands.entity(entity).despawn();
        story.notice = Some(format!(
            "The pick rings and the seam gives. You carry off {}. Stone: {}.",
            task.yields_text(),
            resources.progression.supply(SupplyId::Stone)
        ));
        return;
    }
    if let Ok(pickup) = queries.pickups.get(entity) {
        let PickupReward::Supply(item, amount) = pickup.reward;
        resources.progression.add_supply(item, amount);
        resources.progression.collect_pickup(&pickup.id);
        target.consumed = true;
        commands.entity(entity).insert(Visibility::Hidden);
        match target.kind {
            InteractableKind::Kindling => {
                story.kindling = story.kindling.saturating_add(1);
                if matches!(
                    story.stage,
                    StoryStage::Arrival | StoryStage::GatherKindling
                ) {
                    story.stage = if story.kindling >= 3 {
                        StoryStage::LightHearth
                    } else {
                        StoryStage::GatherKindling
                    };
                }
                story.notice = Some(format!(
                    "Dry wood, sheltered beneath the old growth. Kindling: {}.",
                    resources.progression.supply(SupplyId::Kindling)
                ));
            }
            InteractableKind::Log => {
                story.notice = Some(format!(
                    "A fallen log, weathered but sound. Logs: {}.",
                    resources.progression.supply(SupplyId::Log)
                ));
            }
            InteractableKind::Plank => {
                if story.stage == StoryStage::FindPlank {
                    story.stage = StoryStage::RestoreDesk;
                }
                story.notice = Some(format!(
                    "Old cedar, still sound. Planks: {}.",
                    resources.progression.supply(SupplyId::Plank)
                ));
            }
            InteractableKind::Tool => {
                story.notice = Some(
                    "An old ladder, silvered by weather but still sturdy. It can reach the motel roof."
                        .to_owned(),
                );
            }
            _ => {}
        }
        return;
    }
    match target.kind {
        InteractableKind::Sign => {
            story.notice = Some(
                "MOT—L. A shelter-name from the old speech. An arrow points into the court."
                    .to_owned(),
            );
            if story.stage == StoryStage::Arrival {
                story.stage = StoryStage::GatherKindling;
            }
        }
        InteractableKind::Hearth if story.stage == StoryStage::LightHearth => {
            if resources
                .interior_state
                .0
                .get("motel-exterior/tall-chimney-01")
                .is_none_or(|state| state != "repaired")
            {
                story.notice = Some(
                    "Soot and old nests choke the flue. Clear the office chimney from the roof before lighting a fire."
                        .to_owned(),
                );
                return;
            }
            if !resources
                .progression
                .spend_supply(SupplyId::Kindling, HEARTH_KINDLING)
            {
                story.notice = Some(format!(
                    "The hearth needs {HEARTH_KINDLING} kindling; you have {}.",
                    resources.progression.supply(SupplyId::Kindling)
                ));
                return;
            }
            if let (Ok((mut instance, mut sprite, mut transform, mut visibility)), Some(element)) = (
                queries.mutable_elements.get_mut(entity),
                interior.mutable_element("stone-fireplace-1-01"),
            ) {
                let center = element.states.get("repaired").map_or_else(
                    || transform.translation.truncate(),
                    |visual| interior.element_center(element, visual.size),
                );
                repair_scene_element(
                    &asset_server,
                    &mut resources.interior_state,
                    element,
                    center,
                    &mut instance,
                    &mut sprite,
                    &mut transform,
                    &mut visibility,
                );
            }
            story.stage = if bible_found(&resources.interior_state) {
                StoryStage::FindPlank
            } else {
                StoryStage::FindBible
            };
            target.consumed = true;
            story.notice = Some(
                "Flame takes. Warm light reaches into a room untouched for centuries.".to_owned(),
            );
        }
        InteractableKind::Desk => {
            if repair_pressed && story.stage == StoryStage::RestoreDesk {
                if let (
                    Ok((mut instance, mut sprite, mut transform, mut visibility)),
                    Some(element),
                ) = (
                    queries.mutable_elements.get_mut(entity),
                    interior.mutable_element("old-desk-01"),
                ) {
                    let _outcome = match resources.progression.attempt(&element.task) {
                        Ok(outcome) => outcome,
                        Err(reason) => {
                            story.notice = Some(format!(
                                "You cannot {} the {} yet. {}",
                                element.task.action.infinitive(),
                                element.label,
                                reason
                            ));
                            return;
                        }
                    };
                    let center = element.states.get("repaired").map_or_else(
                        || transform.translation.truncate(),
                        |visual| interior.element_center(element, visual.size),
                    );
                    repair_scene_element(
                        &asset_server,
                        &mut resources.interior_state,
                        element,
                        center,
                        &mut instance,
                        &mut sprite,
                        &mut transform,
                        &mut visibility,
                    );
                    start_task_animation(
                        &element.task,
                        &mut commands,
                        &asset_server,
                        &mut queries.player_animation,
                    );
                }
                story.stage = StoryStage::Night;
                story.notice = Some(
                    "The desk stands square again; the first careful carpentry lesson is learned."
                        .to_owned(),
                );
                target.consumed = true;
                return;
            }
            let mut discoveries = Vec::new();
            if !resources.motel_access.keys_found {
                resources.motel_access.keys_found = true;
                resources
                    .progression
                    .add_supply(SupplyId::Nails, DESK_DRAWER_NAILS);
                discoveries.push(
                    "A ring of numbered brass keys and twelve usable nails wait in the desk's shallow drawer. The other motel doors can now be opened."
                        .to_owned(),
                );
            }
            present_story_beat(
                &mut popup,
                &mut resources.interior_state,
                StoryBeat::OfficeLedger,
            );
            reconcile_office_realization(&mut popup, &mut resources.interior_state);
            story.notice = Some(if discoveries.is_empty() {
                "The old desk has already yielded its secrets.".to_owned()
            } else {
                discoveries.join("\n\n")
            });
        }
        InteractableKind::BibleNightstand => {
            record_bible_discovery(&mut story, &mut resources.interior_state);
            popup.present(NarrativeCard::Item(DiscoveredItem::GideonBible));
            story.notice = Some(
                "The little book remains safe on the nightstand. Someone left it here for a stranger—and the stranger can read."
                    .to_owned(),
            );
        }
        InteractableKind::Traveler if story.stage == StoryStage::MeetTraveler => {
            story.stage = StoryStage::Dialogue;
            story.dialogue_line = 0;
            story.notice = None;
        }
        InteractableKind::MotelDoor | InteractableKind::InteriorExit => {}
        InteractableKind::InteriorRepairable => {
            let Ok((mut instance, mut sprite, mut transform, mut visibility)) =
                queries.mutable_elements.get_mut(entity)
            else {
                return;
            };
            let Some(element) = interior.mutable_element(&instance.id) else {
                return;
            };
            let center = element.states.get("repaired").map_or_else(
                || transform.translation.truncate(),
                |visual| interior.element_center(element, visual.size),
            );
            let outcome = match resources.progression.attempt(&element.task) {
                Ok(outcome) => outcome,
                Err(reason) => {
                    story.notice = Some(format!(
                        "You cannot {} the {} yet. {}\nRequires: {}.",
                        element.task.action.infinitive(),
                        element.label,
                        reason,
                        element.task.requirements_text()
                    ));
                    return;
                }
            };
            if !repair_scene_element(
                &asset_server,
                &mut resources.interior_state,
                element,
                center,
                &mut instance,
                &mut sprite,
                &mut transform,
                &mut visibility,
            ) {
                story.notice = Some(format!("{} cannot be repaired yet.", element.label));
                return;
            }
            start_task_animation(
                &element.task,
                &mut commands,
                &asset_server,
                &mut queries.player_animation,
            );
            target.consumed = true;
            story.notice = Some(task_success_notice(element, &outcome));
        }
        InteractableKind::ExteriorRepairable => {
            let Ok((mut instance, mut sprite, mut transform, mut visibility)) =
                queries.mutable_elements.get_mut(entity)
            else {
                return;
            };
            let scene_id = instance.scene_id.clone();
            let (element, center) = if scene_id == motel.id() {
                let Some(element) = motel.mutable_element(&instance.id) else {
                    return;
                };
                let center = element.states.get("repaired").map_or_else(
                    || transform.translation.truncate(),
                    |visual| motel.element_center(element, visual.size),
                );
                (element, center)
            } else if scene_id == tool_shed.id() {
                let Some(element) = tool_shed.mutable_element(&instance.id) else {
                    return;
                };
                let center = element.states.get("repaired").map_or_else(
                    || transform.translation.truncate(),
                    |visual| tool_shed.element_center(element, visual.size),
                );
                (element, center)
            } else {
                return;
            };
            let outcome = match resources.progression.attempt(&element.task) {
                Ok(outcome) => outcome,
                Err(reason) => {
                    story.notice = Some(format!(
                        "You cannot {} the {} yet. {}\nRequires: {}.",
                        element.task.action.infinitive(),
                        element.label,
                        reason,
                        element.task.requirements_text()
                    ));
                    return;
                }
            };
            if !repair_scene_element(
                &asset_server,
                &mut resources.interior_state,
                element,
                center,
                &mut instance,
                &mut sprite,
                &mut transform,
                &mut visibility,
            ) {
                story.notice = Some(format!("{} cannot be repaired yet.", element.label));
                return;
            }
            start_task_animation(
                &element.task,
                &mut commands,
                &asset_server,
                &mut queries.player_animation,
            );
            target.consumed = true;
            story.notice = Some(task_success_notice(element, &outcome));
        }
        _ => {
            story.notice = Some("There may be a use for this later.".to_owned());
        }
    }
}

fn task_success_notice(
    element: &interior::MutableElement,
    outcome: &progression::TaskOutcome,
) -> String {
    let mut notice = format!(
        "You {} the {}. +{} {} experience.",
        element.task.action.past_tense(),
        element.label,
        element.task.xp,
        element.task.skill.label()
    );
    if !element.task.yields.is_empty() {
        let _ = write!(notice, "\nSalvaged: {}.", element.task.yields_text());
    }
    if outcome.new_level > outcome.old_level {
        let _ = write!(
            notice,
            "\n{} rises to level {}!",
            element.task.skill.label(),
            outcome.new_level
        );
    }
    notice
}

#[allow(clippy::too_many_arguments)]
fn repair_scene_element(
    asset_server: &AssetServer,
    interior_state: &mut InteriorState,
    element: &interior::MutableElement,
    center: Vec2,
    instance: &mut MutableSceneElement,
    sprite: &mut Sprite,
    transform: &mut Transform,
    visibility: &mut Visibility,
) -> bool {
    let Some(repaired) = element.states.get("repaired") else {
        return false;
    };
    if let Some(path) = &repaired.image_path {
        sprite.image = asset_server.load(path.clone());
    }
    sprite.custom_size = Some(repaired.size.max(Vec2::ONE));
    transform.translation.x = center.x;
    transform.translation.y = center.y;
    *visibility = if repaired.visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    "repaired".clone_into(&mut instance.state);
    interior_state.0.insert(
        format!("{}/{}", instance.scene_id, instance.id),
        instance.state.clone(),
    );
    true
}

fn handle_story_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut story: ResMut<Story>,
    mut popup: ResMut<NarrativePopup>,
    inbox: Res<InterpretInbox>,
) {
    if popup.is_open() {
        popup.handle_input(
            keys.just_pressed(KeyCode::KeyE)
                || keys.just_pressed(KeyCode::Space)
                || keys.just_pressed(KeyCode::Escape),
        );
        return;
    }
    if story.stage == StoryStage::Night && keys.just_pressed(KeyCode::Space) {
        story.stage = StoryStage::MeetTraveler;
        story.notice = Some(
            "At first light, a figure follows the thread of smoke down from the ridge.".to_owned(),
        );
        return;
    }
    if story.stage == StoryStage::Dialogue && keys.just_pressed(KeyCode::Space) {
        let vignette = &vignettes()[story.vignette_index];
        story.dialogue_line += 1;
        if story.dialogue_line >= vignette.lines.len() {
            story.stage = StoryStage::Interpreting;
            begin_interpretation(story.vignette_id(), &inbox);
        }
        return;
    }
    let choice = if keys.just_pressed(KeyCode::Digit1) {
        Some(1)
    } else if keys.just_pressed(KeyCode::Digit2) {
        Some(2)
    } else if keys.just_pressed(KeyCode::Digit3) {
        Some(3)
    } else {
        None
    };
    if let Some(choice) = choice {
        match story.stage {
            StoryStage::ChoosePaper => {
                story.card.paper = choice;
                story.stage = StoryStage::ChooseIllustration;
            }
            StoryStage::ChooseIllustration => {
                story.card.illustration = choice;
                story.stage = StoryStage::ChooseBorder;
            }
            StoryStage::ChooseBorder => {
                story.card.border = choice;
                story.stage = StoryStage::FinishedCard;
            }
            _ => {}
        }
    }
    if story.stage == StoryStage::FinishedCard && keys.just_pressed(KeyCode::KeyE) {
        story.stage = StoryStage::Epilogue;
    }
    if story.stage == StoryStage::Epilogue && keys.just_pressed(KeyCode::Space) {
        story.reset_for_replay();
    }
}

fn begin_interpretation(vignette_id: &str, inbox: &InterpretInbox) {
    if let Ok(mut value) = inbox.0.lock() {
        *value = None;
    }
    let inbox = Arc::clone(&inbox.0);
    let request = InterpretRequest {
        vignette_id: vignette_id.to_owned(),
    };
    let body = match serde_json::to_vec(&request) {
        Ok(body) => body,
        Err(error) => {
            if let Ok(mut slot) = inbox.lock() {
                *slot = Some(Err(error.to_string()));
            }
            return;
        }
    };
    let url = api_url("/api/interpret");
    let mut request = ehttp::Request::post(url, body);
    request.headers = ehttp::Headers::new(&[
        ("Accept", "application/json"),
        ("Content-Type", "application/json"),
    ]);
    ehttp::fetch(request, move |response| {
        let result = response.and_then(|response| {
            if response.ok {
                serde_json::from_slice::<InterpretResponse>(&response.bytes)
                    .map_err(|error| error.to_string())
            } else {
                Err(format!(
                    "interpretation service returned {}",
                    response.status
                ))
            }
        });
        if let Ok(mut slot) = inbox.lock() {
            *slot = Some(result);
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn api_url(path: &str) -> String {
    path.to_owned()
}

#[cfg(not(target_arch = "wasm32"))]
fn api_url(path: &str) -> String {
    format!("http://127.0.0.1:7777{path}")
}

fn poll_interpretation(mut story: ResMut<Story>, inbox: Res<InterpretInbox>) {
    if story.stage != StoryStage::Interpreting {
        return;
    }
    let result = inbox.0.lock().ok().and_then(|mut slot| slot.take());
    let Some(result) = result else {
        return;
    };
    let response = match result {
        Ok(response) => response,
        Err(error) => {
            bevy::log::warn!("API request failed: {error}; using reviewed fixture");
            fixture_response(story.vignette_id()).expect("every vignette has a fixture")
        }
    };
    story.result = Some(response);
    story.stage = StoryStage::ChoosePaper;
}

fn sync_world_state(
    story: Res<Story>,
    mut traveler: Query<(&mut Visibility, &mut Transform), With<Traveler>>,
) {
    if !story.is_changed() {
        return;
    }
    if let Ok((mut visibility, mut transform)) = traveler.single_mut() {
        *visibility = if matches!(
            story.stage,
            StoryStage::MeetTraveler
                | StoryStage::Dialogue
                | StoryStage::Interpreting
                | StoryStage::ChoosePaper
                | StoryStage::ChooseIllustration
                | StoryStage::ChooseBorder
                | StoryStage::FinishedCard
        ) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        transform.translation.x = -70.0;
    }
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_ui(
    story: Res<Story>,
    progression: Res<Progression>,
    ui_knowledge: UiKnowledge,
    nearby: Res<Nearby>,
    asset_server: Res<AssetServer>,
    interactables: Query<&Interactable>,
    interaction_details: Query<(
        Option<&AuthoredInteractionLabel>,
        Option<&PortableToolEntity>,
    )>,
    task_targets: Query<&TaskTarget>,
    mut progress_text: Query<&mut Text, With<ProgressText>>,
    mut status: Query<
        &mut TextSpan,
        (
            With<StatusText>,
            Without<PromptText>,
            Without<OverlayTitle>,
            Without<OverlayBody>,
            Without<ProvenanceText>,
        ),
    >,
    mut prompt: Query<
        &mut TextSpan,
        (
            With<PromptText>,
            Without<StatusText>,
            Without<OverlayTitle>,
            Without<OverlayBody>,
            Without<ProvenanceText>,
        ),
    >,
    mut overlay: Query<&mut Visibility, (With<OverlayRoot>, Without<CardArt>)>,
    mut title: Query<
        &mut Text,
        (
            With<OverlayTitle>,
            Without<OverlayBody>,
            Without<StatusText>,
            Without<PromptText>,
            Without<ProvenanceText>,
            Without<ProgressText>,
        ),
    >,
    mut body: Query<
        &mut Text,
        (
            With<OverlayBody>,
            Without<OverlayTitle>,
            Without<StatusText>,
            Without<PromptText>,
            Without<ProvenanceText>,
            Without<ProgressText>,
        ),
    >,
    mut provenance: Query<
        &mut Text,
        (
            With<ProvenanceText>,
            Without<OverlayTitle>,
            Without<OverlayBody>,
            Without<StatusText>,
            Without<PromptText>,
            Without<ProgressText>,
        ),
    >,
    mut card_art: Query<(&mut ImageNode, &mut Visibility), (With<CardArt>, Without<OverlayRoot>)>,
) {
    let objective = match story.stage {
        StoryStage::Arrival => "Explore the standing stones. Find out what this place was.",
        StoryStage::GatherKindling => "Gather dry kindling for the motel hearth.",
        StoryStage::LightHearth => {
            "Clear three pieces of debris, find the old ladder, clear the office chimney, then light the hearth."
        }
        StoryStage::FindBible => "Search the nightstand beside the bed in room 3.",
        StoryStage::FindPlank => "Find a sound plank in the valley for the office desk.",
        StoryStage::RestoreDesk => "Repair the writing desk.",
        StoryStage::MeetTraveler => "Welcome the traveler who followed your smoke.",
        StoryStage::Dialogue => "Listen.",
        StoryStage::Interpreting => "Listen for the need beneath the words.",
        StoryStage::ChoosePaper | StoryStage::ChooseIllustration | StoryStage::ChooseBorder => {
            "Make a remembrance for the traveler."
        }
        StoryStage::FinishedCard => "Give the remembrance to the traveler.",
        StoryStage::Night | StoryStage::Epilogue => "",
    };
    if let Ok(mut text) = status.single_mut() {
        **text = story.notice.as_ref().map_or_else(
            || format!("THE SCRIBE\n{objective}"),
            |notice| format!("THE SCRIBE\n{objective}\n\n{notice}"),
        );
    }

    if let Ok(mut text) = progress_text.single_mut() {
        let supplies = progression.supplies_summary();
        let mut knowledge = Vec::new();
        if ui_knowledge.motel_access.keys_found {
            knowledge.push("Numbered motel keys — office");
        }
        if bible_found(&ui_knowledge.interior_state) {
            knowledge.push("Old Gideon Bible — room 3");
        }
        let knowledge = if knowledge.is_empty() {
            String::new()
        } else {
            format!("\n\nKNOWN\n{}", knowledge.join("\n"))
        };
        **text = format!(
            "RESTORATION\n{}\n\nTOOLS\n{}\n\nSUPPLIES\n{}{knowledge}",
            progression.skill_tree_summary(),
            progression.tools_summary(),
            if supplies.is_empty() {
                "none yet"
            } else {
                &supplies
            }
        );
    }

    let nearby_prompt = nearby
        .0
        .and_then(|entity| interactables.get(entity).ok().map(|item| (entity, item)))
        .map_or_else(
            || "Move: WASD/arrows  ·  E interact  ·  R work  ·  Tab tool  ·  Q drop".to_owned(),
            |(entity, item)| {
                if let Ok(task) = task_targets.get(entity) {
                    // Standing stations are named; anonymous scenery is not.
                    let work = match item.kind {
                        InteractableKind::Sawbuck => "saw a log into planks".to_owned(),
                        InteractableKind::StoneOutcrop => {
                            "work stone out of this outcrop".to_owned()
                        }
                        _ => format!("{} this item", task.action.infinitive()),
                    };
                    return format!("R — {work}     [{}]", task.requirements);
                }
                if item.kind == InteractableKind::BibleNightstand {
                    return interaction_details
                        .get(entity)
                        .ok()
                        .and_then(|(label, _)| label)
                        .map_or_else(
                            || "E — search here".to_owned(),
                            |label| format!("E — search the {}", label.0),
                        );
                }
                if let Ok((label, Some(portable))) = interaction_details.get(entity) {
                    let label = label.map_or("portable tool", |label| label.0.as_str());
                    return progression.tool_record(&portable.id).map_or_else(
                        || format!("E — take the {label}"),
                        |record| {
                            if record.condition == ToolCondition::Broken {
                                format!("E — take the broken {label}     R — repair it")
                            } else {
                                format!("E — take the {label}")
                            }
                        },
                    );
                }
                match item.kind {
                    InteractableKind::Sign => "E — inspect the old sign",
                    InteractableKind::Kindling => "E — gather kindling",
                    InteractableKind::Log => "E — gather a fallen log",
                    InteractableKind::Hearth => "E — tend the hearth",
                    InteractableKind::Plank => "E — take the sound plank",
                    InteractableKind::Tool => "E — take this tool",
                    InteractableKind::Desk if story.stage == StoryStage::RestoreDesk => {
                        "E — search the old desk     R — repair it"
                    }
                    InteractableKind::Desk => "E — search the old desk",
                    InteractableKind::BibleNightstand => "E — search here",
                    InteractableKind::Traveler => "E — welcome the traveler",
                    InteractableKind::MotelDoor => "Walk through the motel door",
                    InteractableKind::InteriorExit => "Walk onto the exit to step outside",
                    InteractableKind::InteriorRepairable => "R — restore this part of the room",
                    InteractableKind::ExteriorRepairable => "R — restore this part of the building",
                    InteractableKind::Tree => "R — chop this tree",
                    InteractableKind::Sawbuck => "R — saw a log into planks",
                    InteractableKind::StoneOutcrop => "R — work stone out of this outcrop",
                }
                .to_owned()
            },
        );
    if let Ok(mut text) = prompt.single_mut() {
        **text = nearby_prompt;
    }

    let overlay_content: Option<(String, String, String)> = match story.stage {
        StoryStage::Night => Some((
            "A Fire in the Valley".to_owned(),
            "You brace the desk with old cedar. In room 3, the little book waits where the dry walls have guarded it for generations. Smoke rises through a chimney that has been cold longer than any remembered name.\n\nSPACE — sleep until morning"
                .to_owned(),
            String::new(),
        )),
        StoryStage::Dialogue => {
            let vignette = &vignettes()[story.vignette_index];
            let line = &vignette.lines[story.dialogue_line.min(vignette.lines.len() - 1)];
            Some((
                vignette.traveler_name.clone(),
                format!("“{line}”\n\nSPACE — listen"),
                String::new(),
            ))
        }
        StoryStage::Interpreting => Some((
            "The Scribe Listens".to_owned(),
            "The traveler's words settle beside what you have been reading in room 3. You search for the need beneath them…"
                .to_owned(),
            "Gloo AI is selecting from a reviewed passage catalog.".to_owned(),
        )),
        StoryStage::ChoosePaper => Some((
            "I · Prepare the Leaf".to_owned(),
            "Choose the ground that will carry the words.\n\n1  Warm flax    2  Pale cotton    3  Ash-grey rag"
                .to_owned(),
            selection_provenance(&story),
        )),
        StoryStage::ChooseIllustration => Some((
            "II · Choose an Illumination".to_owned(),
            "Choose the small image beside the words.\n\n1  Lamp on the road    2  Shelter tree    3  Open hands"
                .to_owned(),
            selection_provenance(&story),
        )),
        StoryStage::ChooseBorder => Some((
            "III · Mark the Border".to_owned(),
            "Choose how this remembrance will endure.\n\n1  Simple rule    2  Flowering vine    3  Old stone"
                .to_owned(),
            selection_provenance(&story),
        )),
        StoryStage::FinishedCard => story.result.as_ref().map(|result| {
            (
                format!("A Remembrance for {}", story.traveler_name()),
                format!(
                    "{}\n\n“{}”\n\n{}\n\nPaper {} · Illumination {} · Border {}\n\nE — give the remembrance",
                    result.need_label,
                    result.passage.content,
                    result.passage.reference,
                    story.card.paper,
                    story.card.illustration,
                    story.card.border
                ),
                selection_provenance(&story),
            )
        }),
        StoryStage::Epilogue => Some((
            "The First Word Carried".to_owned(),
            format!(
                "{} reads the marks slowly after you speak them aloud. The card disappears into a weathered coat, close to the heart.\n\nBy evening there are new footprints on the old road. Tomorrow, perhaps, there will be another column of smoke answering yours.\n\nSPACE — begin again with another traveler",
                story.traveler_name()
            ),
            "The Waystation at the Edge of the Ash · Scripture via YouVersion · Interpretation via Gloo AI Studio"
                .to_owned(),
        )),
        _ => None,
    };
    if let Ok(mut visibility) = overlay.single_mut() {
        *visibility = if overlay_content.is_some() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if let Some((heading, content, credit)) = overlay_content {
        if let Ok(mut text) = title.single_mut() {
            **text = heading;
        }
        if let Ok(mut text) = body.single_mut() {
            **text = content;
        }
        if let Ok(mut text) = provenance.single_mut() {
            **text = credit;
        }
    }
    if let Ok((mut image, mut visibility)) = card_art.single_mut() {
        *visibility = if matches!(
            story.stage,
            StoryStage::ChooseIllustration | StoryStage::ChooseBorder | StoryStage::FinishedCard
        ) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        let motif = story
            .result
            .as_ref()
            .map_or(1, |result| match result.need_id.as_str() {
                "rest" | "courage" => 2,
                "belonging" | "mercy" => 3,
                _ => 1,
            });
        image.image = asset_server.load(format!(
            "card/illustration_{motif}_{}.png",
            story.card.illustration
        ));
    }
}

#[allow(clippy::type_complexity)]
fn sync_narrative_popup_ui(
    popup: Res<NarrativePopup>,
    asset_server: Res<AssetServer>,
    mut root: Query<&mut Visibility, With<NarrativePopupRoot>>,
    mut text: Query<
        (
            &mut Text,
            Option<&NarrativePopupTitle>,
            Option<&NarrativePopupBody>,
        ),
        (
            Or<(With<NarrativePopupTitle>, With<NarrativePopupBody>)>,
            Without<NarrativePopupDismiss>,
        ),
    >,
    mut art: Query<(&mut ImageNode, &mut Node), With<NarrativePopupArt>>,
    mut dismiss: Query<
        &mut Text,
        (
            With<NarrativePopupDismiss>,
            Without<NarrativePopupTitle>,
            Without<NarrativePopupBody>,
        ),
    >,
) {
    let Some(card) = popup.current else {
        if let Ok(mut visibility) = root.single_mut() {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    if let Ok(mut visibility) = root.single_mut() {
        *visibility = Visibility::Visible;
    }
    for (mut value, title, body) in &mut text {
        if title.is_some() {
            card.title().clone_into(&mut **value);
        } else if body.is_some() {
            card.description().clone_into(&mut **value);
        }
    }
    if let Ok((mut image, mut node)) = art.single_mut() {
        if let Some(path) = card.image_path() {
            image.image = asset_server.load(path);
            node.display = Display::Flex;
        } else {
            node.display = Display::None;
        }
    }
    if let Ok(mut text) = dismiss.single_mut() {
        card.dismiss_label().clone_into(&mut **text);
    }
}

fn selection_provenance(story: &Story) -> String {
    story.result.as_ref().map_or_else(String::new, |result| {
        format!(
            "{} via YouVersion · {} / {} via Gloo AI Studio · source: {:?}",
            result.passage.version,
            result.provenance.gloo_model,
            result.provenance.routing,
            result.provenance.scripture_source
        )
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::progression::SkillId;

    #[test]
    fn replay_rotates_authored_travelers() {
        let mut story = Story::default();
        assert_eq!(story.vignette_id(), "mara_grief");
        story.reset_for_replay();
        assert_eq!(story.vignette_id(), "oren_weariness");
    }

    #[test]
    fn every_story_vignette_is_registered() {
        for item in vignettes() {
            assert!(waystation_shared::vignette(&item.id).is_some());
            assert!(fixture_response(&item.id).is_some());
        }
    }

    #[test]
    fn save_data_keeps_mutable_room_state_by_stable_id() {
        let story = Story::default();
        let mut interior_state = InteriorState::default();
        interior_state
            .0
            .insert("motel-room-01/mirror-01".to_owned(), "repaired".to_owned());
        interior_state
            .0
            .insert(BIBLE_STATE_KEY.to_owned(), DISCOVERY_FOUND_STATE.to_owned());

        let motel_access = MotelAccess { keys_found: true };
        let mut progression = Progression::default();
        progression.add_tool(ToolId::Hammer);
        progression.add_supply(SupplyId::Plank, 2);
        let save = SaveData::capture(&story, &interior_state, &motel_access, &progression);

        assert_eq!(save.version, 6);
        assert_eq!(save.interior_states["motel-room-01/mirror-01"], "repaired");
        assert!(bible_found(&InteriorState(save.interior_states.clone())));
        assert!(save.motel_keys_found);
        assert!(save.progression.has_tool(ToolId::Hammer));
        assert_eq!(save.progression.supply(SupplyId::Plank), 2);
    }

    #[test]
    fn repairables_use_r_while_search_and_interaction_use_e() {
        assert!(interaction_key_matches(
            InteractableKind::InteriorRepairable,
            StoryStage::FindBible,
            false,
            true,
        ));
        assert!(!interaction_key_matches(
            InteractableKind::InteriorRepairable,
            StoryStage::FindBible,
            true,
            false,
        ));
        assert!(interaction_key_matches(
            InteractableKind::BibleNightstand,
            StoryStage::FindBible,
            true,
            false,
        ));
        assert!(!interaction_key_matches(
            InteractableKind::BibleNightstand,
            StoryStage::FindBible,
            false,
            true,
        ));
        assert!(interaction_key_matches(
            InteractableKind::Desk,
            StoryStage::RestoreDesk,
            false,
            true,
        ));
    }

    #[test]
    fn completed_legacy_story_stages_imply_the_bible_was_already_found() {
        assert!(!story_stage_requires_bible(StoryStage::FindBible));
        assert!(story_stage_requires_bible(StoryStage::FindPlank));
        assert!(story_stage_requires_bible(StoryStage::Epilogue));
    }

    #[test]
    fn finding_the_room_three_bible_persists_and_advances_its_story_beat() {
        let mut story = Story {
            stage: StoryStage::FindBible,
            ..Story::default()
        };
        let mut interior_state = InteriorState::default();

        record_bible_discovery(&mut story, &mut interior_state);

        assert!(bible_found(&interior_state));
        assert_eq!(story.stage, StoryStage::FindPlank);
    }

    #[test]
    fn narrative_popup_waits_for_a_later_keypress_before_dismissal() {
        let mut popup = NarrativePopup::default();

        popup.present(NarrativeCard::Item(DiscoveredItem::GideonBible));
        popup.handle_input(true);
        assert!(popup.is_open());

        popup.handle_input(false);
        assert!(popup.is_open());

        popup.handle_input(true);
        assert!(!popup.is_open());
    }

    #[test]
    fn narrative_popup_queues_story_beats_without_skipping_them() {
        let mut popup = NarrativePopup::default();
        popup.present(NarrativeCard::Thought(StoryBeat::OfficeLedger));
        popup.present(NarrativeCard::Thought(StoryBeat::OfficeWelcome));

        assert_eq!(
            popup.current,
            Some(NarrativeCard::Thought(StoryBeat::OfficeLedger))
        );
        popup.handle_input(false);
        popup.handle_input(true);
        assert_eq!(
            popup.current,
            Some(NarrativeCard::Thought(StoryBeat::OfficeWelcome))
        );
    }

    #[test]
    fn office_welcome_realization_requires_all_three_observations() {
        let mut popup = NarrativePopup::default();
        let mut interior_state = InteriorState::default();

        present_story_beat(&mut popup, &mut interior_state, StoryBeat::OfficeThreshold);
        present_story_beat(&mut popup, &mut interior_state, StoryBeat::OfficeLedger);
        reconcile_office_realization(&mut popup, &mut interior_state);
        assert!(!story_beat_seen(&interior_state, StoryBeat::OfficeWelcome));

        present_story_beat(&mut popup, &mut interior_state, StoryBeat::OfficeHearth);
        reconcile_office_realization(&mut popup, &mut interior_state);
        assert!(story_beat_seen(&interior_state, StoryBeat::OfficeWelcome));
    }

    #[test]
    fn authored_motel_doors_route_left_to_right_from_office_through_room_six() {
        let motel = interior::MotelExteriorMap::load();
        let routes = motel_door_routes(&motel);

        assert_eq!(routes["damaged-door-1-01"], interior::InteriorId::Office);
        assert_eq!(routes["damaged-door-2-01"], interior::InteriorId::Room01);
        assert_eq!(routes["damaged-door-4-01"], interior::InteriorId::Room05);
        assert_eq!(routes["damaged-door-2-03"], interior::InteriorId::Room06);
    }

    #[test]
    fn office_and_room_five_start_open_and_desk_keys_unlock_the_rest() {
        let locked_door = MotelDoorDestination {
            interior_id: interior::InteriorId::Room01,
            initially_unlocked: false,
            doorstep: Vec2::ZERO,
        };
        let open_door = MotelDoorDestination {
            interior_id: interior::InteriorId::Room05,
            initially_unlocked: true,
            doorstep: Vec2::ZERO,
        };
        let mut access = MotelAccess::default();

        assert!(motel_door_is_unlocked(open_door, &access));
        assert!(!motel_door_is_unlocked(locked_door, &access));
        access.keys_found = true;
        assert!(motel_door_is_unlocked(locked_door, &access));
    }

    #[test]
    fn automatic_door_requires_the_player_to_be_inside_its_art_bounds() {
        let door = Vec2::new(100.0, 50.0);
        let size = Vec2::new(60.0, 78.0);

        assert!(player_inside_doorway(
            door + Vec2::new(29.0, 38.0),
            door,
            size
        ));
        assert!(!player_inside_doorway(
            door + Vec2::new(31.0, 0.0),
            door,
            size
        ));
        assert!(!player_inside_doorway(
            door + Vec2::new(0.0, -40.0),
            door,
            size
        ));
    }

    #[test]
    fn office_transition_uses_the_authored_collision_above_the_door() {
        let motel = interior::MotelExteriorMap::load();
        let door = motel
            .mutable_element("damaged-door-1-01")
            .expect("office door");
        let visual = &door.states["damaged"];
        let door_center = motel.element_center(door, visual.size);
        let player = (0..=128_u16)
            .map(|step| door_center + Vec2::new(0.0, -visual.size.y / 2.0 + f32::from(step)))
            .find(|candidate| {
                let stance = player_collision_rect(*candidate);
                player_inside_doorway(*candidate, door_center, visual.size)
                    && motel.is_area_walkable(stance.center, stance.size)
                    && !motel.is_walkable(*candidate + Vec2::Y * DOOR_HEAD_PROBE_OFFSET)
            })
            .expect("office doorway needs a walkable approach below its collision threshold");

        assert!(player_inside_doorway(player, door_center, visual.size));
        let stance = player_collision_rect(player);
        assert!(motel.is_area_walkable(stance.center, stance.size));
        assert!(!motel.is_walkable(player + Vec2::Y * DOOR_HEAD_PROBE_OFFSET));
    }

    #[test]
    fn locked_door_bump_rearms_only_after_forward_is_released() {
        let mut latch = None;

        assert!(latch_locked_door_bump(
            &mut latch,
            interior::InteriorId::Room01
        ));
        assert!(!latch_locked_door_bump(
            &mut latch,
            interior::InteriorId::Room01
        ));
        latch = None;
        assert!(latch_locked_door_bump(
            &mut latch,
            interior::InteriorId::Room01
        ));
    }

    /// Every restoration job the content authors, with the supplies it spends
    /// and the tools it asks for.
    fn authored_restoration_tasks() -> Vec<progression::TaskSpec> {
        let mut tasks = Vec::new();
        for interior_id in interior::InteriorId::ALL {
            let room = interior::InteriorMap::load(interior_id);
            tasks.extend(room.mutable_elements().iter().map(|e| e.task.clone()));
        }
        let motel = interior::MotelExteriorMap::load();
        let tool_shed = interior::ToolShedExteriorMap::load();
        tasks.extend(motel.mutable_elements().iter().map(|e| e.task.clone()));
        tasks.extend(tool_shed.mutable_elements().iter().map(|e| e.task.clone()));
        tasks
    }

    fn total_demand(tasks: &[progression::TaskSpec], item: SupplyId) -> u32 {
        tasks
            .iter()
            .flat_map(|task| &task.supplies)
            .filter(|cost| cost.item == item)
            .map(|cost| u32::from(cost.amount))
            .sum()
    }

    fn total_yield(tasks: &[progression::TaskSpec], item: SupplyId) -> u32 {
        tasks
            .iter()
            .flat_map(|task| &task.yields)
            .filter(|gain| gain.item == item)
            .map(|gain| u32::from(gain.amount))
            .sum()
    }

    fn count(placements: usize) -> u32 {
        u32::try_from(placements).expect("authored placement tables are small")
    }

    fn yield_per_task(task: &progression::TaskSpec, item: SupplyId) -> u32 {
        task.yields
            .iter()
            .filter(|gain| gain.item == item)
            .map(|gain| u32::from(gain.amount))
            .sum()
    }

    /// The failure this guards against is silent: a task can be authored with a
    /// supply or tool the valley never produces, and the skill it belongs to
    /// then sits at zero forever with nothing in the UI to say why.
    #[test]
    fn the_valley_can_pay_for_every_authored_restoration_task() {
        let tasks = authored_restoration_tasks();
        let milling = progression::TaskSpec::for_milling();
        let quarrying = progression::TaskSpec::for_quarrying();
        let chopping = progression::TaskSpec::for_tree_chopping();

        let logs = count(LOG_PICKUPS.len())
            + count(TREE_PLACEMENTS.len()) * yield_per_task(&chopping, SupplyId::Log);
        let planks = count(PLANK_PICKUPS.len())
            + logs * yield_per_task(&milling, SupplyId::Plank)
                / u32::from(milling.supplies[0].amount);
        let stone =
            count(STONE_OUTCROP_PLACEMENTS.len()) * yield_per_task(&quarrying, SupplyId::Stone);
        let nails = u32::from(DESK_DRAWER_NAILS) + total_yield(&tasks, SupplyId::Nails);
        let kindling = count(KINDLING_PICKUPS.len())
            + count(TREE_PLACEMENTS.len()) * yield_per_task(&chopping, SupplyId::Kindling);

        assert!(planks >= total_demand(&tasks, SupplyId::Plank));
        assert!(stone >= total_demand(&tasks, SupplyId::Stone));
        assert!(nails >= total_demand(&tasks, SupplyId::Nails));
        assert!(kindling >= total_demand(&tasks, SupplyId::Kindling) + u32::from(HEARTH_KINDLING));
        assert_eq!(total_demand(&tasks, SupplyId::Log), 0);
    }

    #[test]
    fn every_tool_an_authored_task_asks_for_exists_in_the_valley() {
        let mut obtainable: BTreeSet<ToolId> =
            interior::InteriorMap::load(interior::InteriorId::ToolShed)
                .portable_items()
                .iter()
                // A broken tool still counts: the Scribe can repair it in place.
                .map(|item| item.tool)
                .collect();
        obtainable.insert(ToolId::Ladder);

        let mut required: BTreeSet<ToolId> = authored_restoration_tasks()
            .iter()
            .flat_map(|task| task.tools.clone())
            .collect();
        for station in [
            progression::TaskSpec::for_milling(),
            progression::TaskSpec::for_quarrying(),
            progression::TaskSpec::for_tree_chopping(),
        ] {
            required.extend(station.tools);
        }

        assert!(
            required.is_subset(&obtainable),
            "unobtainable tools: {:?}",
            required.difference(&obtainable).collect::<Vec<_>>()
        );
    }

    /// Reaching a skill's ceiling has to be possible from tasks whose own level
    /// requirement is already met, or the tree deadlocks below the top.
    #[test]
    fn every_skill_has_enough_authored_work_to_reach_its_ceiling() {
        let tasks = authored_restoration_tasks();
        for skill in SkillId::ALL {
            let reachable_xp: u16 = tasks
                .iter()
                .filter(|task| task.skill == skill && task.level == 0)
                .map(|task| task.xp)
                .sum();
            assert!(
                u32::from(reachable_xp) >= progression::xp_for_max_level(),
                "{} cannot reach its ceiling: {reachable_xp} experience authored at level 0",
                skill.label()
            );
        }
    }

    #[test]
    fn every_tree_resolves_to_a_land_footprint_and_blocks_its_trunk() {
        let grid = terrain::WorldGrid::generate(terrain::WORLD_SEED);
        let motel = interior::MotelExteriorMap::load();
        let tool_shed = interior::ToolShedExteriorMap::load();
        for (x, y, size) in TREE_PLACEMENTS {
            let position = resolve_tree_position(&grid, &motel, &tool_shed, Vec2::new(x, y), size);
            let ground = tree_ground_rect(position, size);
            let trunk = tree_trunk_rect(position, size);
            assert!(grid.supports_land_footprint(ground.center, ground.size));
            assert!(motel.is_area_walkable(trunk.center, trunk.size));
            assert!(tool_shed.is_area_walkable(trunk.center, trunk.size));

            let obstacles = ExteriorObstacles {
                solid_footprints: vec![tree_trunk_rect(position, size)],
                prop_exclusions: vec![],
            };
            assert!(!obstacles.player_can_stand(player_collision_rect(ground.center)));
        }
    }

    #[test]
    fn player_feet_allow_natural_overlap_but_cannot_enter_water() {
        let grid = terrain::WorldGrid::generate(terrain::WORLD_SEED);
        let water = (1..terrain::MAP_HEIGHT)
            .flat_map(|y| (0..terrain::MAP_WIDTH).map(move |x| (x, y)))
            .find(|&(x, y)| {
                grid.get(x, y) == terrain::Terrain::Water
                    && grid.get(x, y - 1) != terrain::Terrain::Water
            })
            .expect("generated water has a southern land edge");
        let water_x = f32::from(u16::try_from(water.0).expect("world x fits u16"));
        let water_y = f32::from(u16::try_from(water.1).expect("world y fits u16"));
        let water_edge_y = water_y.mul_add(terrain::TILE_SIZE, -MAP_HALF_HEIGHT);
        let x = (water_x + 0.5).mul_add(terrain::TILE_SIZE, -MAP_HALF_WIDTH);
        let overlapping_body = Vec2::new(x, water_edge_y + 8.0);
        let body_overlap_bounds = player_collision_rect(overlapping_body);
        let feet_entering_water = Vec2::new(x, water_edge_y + 12.0);
        let blocked_bounds = player_collision_rect(feet_entering_water);

        assert_ne!(grid.get(water.0, water.1 - 1), terrain::Terrain::Water);
        assert!(grid.supports_land_footprint(body_overlap_bounds.center, body_overlap_bounds.size));
        assert!(!grid.supports_land_footprint(blocked_bounds.center, blocked_bounds.size));
    }

    #[test]
    fn exterior_depth_sorts_by_ground_contact() {
        let tree_ground_y = 100.0;
        let player_behind_ground_y = 120.0;
        let player_in_front_ground_y = 80.0;

        assert!(exterior_depth(player_behind_ground_y) < exterior_depth(tree_ground_y));
        assert!(exterior_depth(player_in_front_ground_y) > exterior_depth(tree_ground_y));
    }

    #[test]
    fn dense_tree_full_hide_uses_the_art_silhouette() {
        let circular_canopy = |point: Vec2| point.length() <= 48.0;

        assert!(player_is_fully_covered_by_tree_alpha(
            Vec2::ZERO,
            circular_canopy,
        ));
        assert!(!player_is_fully_covered_by_tree_alpha(
            Vec2::new(32.0, 0.0),
            circular_canopy,
        ));
    }

    #[test]
    fn tree_world_points_map_to_png_pixels_without_filling_transparent_bounds() {
        let image_size = UVec2::splat(64);
        let tree_position = Vec2::new(100.0, 200.0);

        assert_eq!(
            tree_image_pixel_at_world_point(tree_position, tree_position, 160.0, image_size,),
            Some(UVec2::splat(32))
        );
        assert_eq!(
            tree_image_pixel_at_world_point(
                tree_position + Vec2::new(-79.0, 79.0),
                tree_position,
                160.0,
                image_size,
            ),
            Some(UVec2::ZERO)
        );
        assert_eq!(
            tree_image_pixel_at_world_point(
                tree_position + Vec2::new(80.0, 0.0),
                tree_position,
                160.0,
                image_size,
            ),
            None
        );
    }

    #[test]
    fn building_layers_sort_together_around_their_collision_ground_line() {
        let building_ground_y = 100.0;
        let player_behind = exterior_depth(building_ground_y + 1.0);
        let player_in_front = exterior_depth(building_ground_y - 1.0);
        let floor_baked = exterior_depth(building_ground_y)
            + building_layer_depth_bias(authored_layer_index("floor"), false);
        let overlay_mutable = exterior_depth(building_ground_y)
            + building_layer_depth_bias(authored_layer_index("overlay"), true);

        assert!(player_behind < floor_baked);
        assert!(floor_baked < overlay_mutable);
        assert!(overlay_mutable < player_in_front);
        assert!(building_occlusion_crown_depth(building_ground_y) > overlay_mutable);
        assert!((scribe_occlusion_crown_offset_y() - 24.0).abs() < f32::EPSILON);
    }

    #[test]
    fn interior_scenery_covers_only_a_player_standing_behind_its_floor_line() {
        let lamp_ground_y = 104.0;
        let player_behind = lamp_ground_y + 1.0;
        let player_in_front = lamp_ground_y - 1.0;

        assert!(interior_occluder_covers_player(
            player_behind,
            lamp_ground_y
        ));
        assert!(!interior_occluder_covers_player(
            player_in_front,
            lamp_ground_y
        ));
        assert!(interior_occluder_cover_depth(lamp_ground_y) > INTERIOR_PLAYER_DEPTH);
    }

    #[test]
    fn overlapping_interior_occluders_keep_the_southernmost_in_front() {
        let near = interior_occluder_cover_depth(0.0);
        let far = interior_occluder_cover_depth(64.0);

        assert!(near > far);
        assert!(far > INTERIOR_PLAYER_DEPTH);
    }

    #[test]
    fn pickup_safety_moves_the_motel_wall_log_to_clear_land() {
        let grid = terrain::WorldGrid::generate(terrain::WORLD_SEED);
        let motel = interior::MotelExteriorMap::load();
        let tool_shed = interior::ToolShedExteriorMap::load();
        let obstacles = ExteriorObstacles::default();
        let desired = Vec2::new(-390.0, -80.0);
        let size = Vec2::new(48.0, 34.0);

        assert!(!motel.is_area_walkable(desired, size));
        let actual =
            safe_pickup_position(&grid, &motel, &tool_shed, &obstacles, &[], desired, size);
        assert_ne!(actual, desired);
        assert!(grid.supports_land_footprint(actual, size));
        assert!(motel.is_area_walkable(actual, size));
    }

    #[test]
    fn pickup_safety_avoids_tree_art_and_other_drops() {
        let desired = Vec2::new(1_200.0, 400.0);
        let size = Vec2::new(48.0, 32.0);
        let grid = terrain::WorldGrid::generate(terrain::WORLD_SEED);
        let motel = interior::MotelExteriorMap::load();
        let tool_shed = interior::ToolShedExteriorMap::load();
        let occupied = ExteriorRect::new(desired, Vec2::splat(100.0));
        let obstacles = ExteriorObstacles {
            solid_footprints: vec![],
            prop_exclusions: vec![occupied],
        };

        let actual =
            safe_pickup_position(&grid, &motel, &tool_shed, &obstacles, &[], desired, size);
        assert!(!occupied.overlaps(ExteriorRect::new(actual, size)));

        let reserved = [ExteriorRect::new(actual, size)];
        let second = safe_pickup_position(
            &grid, &motel, &tool_shed, &obstacles, &reserved, actual, size,
        );
        assert!(!reserved[0].overlaps(ExteriorRect::new(second, size)));
    }
}
