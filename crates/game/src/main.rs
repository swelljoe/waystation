//! The Waystation at the Edge of the Ash.

#![allow(clippy::needless_pass_by_value)]

mod cards;
mod chance;
mod daylight;
mod game_audio;
mod garden;
mod interior;
mod npc_art;
mod progression;
mod reading;
mod rehearsal;
mod salvage;
mod terrain;
mod upkeep;
mod visitors;

use std::{
    collections::{HashMap, VecDeque},
    fmt::Write as _,
    sync::{Arc, Mutex},
};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use cards::Collection;
use chance::Chance;
use daylight::Clock;
use garden::{Garden, PlotStage};
use progression::{Progression, SupplyId, TaskAction, ToolCondition, ToolId, ToolLocation};
use reading::Readings;
use salvage::Salvaged;
use serde::{Deserialize, Serialize};
use terrain::{MAP_HALF_HEIGHT, MAP_HALF_WIDTH};
use visitors::{Stage as VisitStage, Visitors};
use waystation_shared::{
    fixture_response, InterpretRequest, InterpretResponse, DEFAULT_BIBLE_ABBREVIATION,
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
const SCRIBE_THRUST_FRAME_SIZE: u32 = 64;
const SCRIBE_THRUST_COLUMNS: u32 = 8;
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
/// Saved state keys for the two halves of that fire: smoke needs somewhere to go
/// before a flame is worth striking.
const OFFICE_HEARTH_STATE_KEY: &str = "motel-office/stone-fireplace-1-01";

/// Every wall section in the office shares this prefix. When all of them are
/// mended the room holds its heat, which is the one place in the game where a
/// finished repair goes on paying.
const OFFICE_WALL_PREFIX: &str = "old-room-wall";
const OFFICE_CHIMNEY_STATE_KEY: &str = "motel-exterior/tall-chimney-01";

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

/// A bed is one parking bay. Where they are and what they look like is authored
/// in `content/buildings/motel-parking.json`; this is only the fallback size for
/// a bay whose art failed to load.
const GARDEN_PLOT_SIZE: Vec2 = Vec2::new(96.0, 96.0);
/// Flat ground art: above the terrain, below every prop and the Scribe, so the
/// beds are walked over rather than walked around.
const GARDEN_PLOT_DEPTH: f32 = 0.0;
/// The motel's own sign, out at the western approach where the Scribe first
/// comes down off the ridge. Sized to the art's native pixels like every other
/// prop, so the lettering stays on its own grid.
const MOTEL_SIGN_POSITION: Vec2 = Vec2::new(-780.0, -245.0);
const MOTEL_SIGN_SIZE: Vec2 = Vec2::new(101.0, 100.0);
/// The motel's own rain butt, staved in, standing just past the east end of the
/// bays where the roofline drains. Nothing in this valley is lying about waiting
/// to be useful; it holds nothing at all until the Scribe puts it together.
const RAIN_CISTERN_POSITION: Vec2 = Vec2::new(480.0, -200.0);
const RAIN_CISTERN_SIZE: Vec2 = Vec2::new(64.0, 64.0);
const RAIN_CISTERN_STATE_KEY: &str = "motel-exterior/rain-cistern";
/// Wild food, and meagre by design. Nothing here is farmed, traded, or left in a
/// sack by somebody else; it is what the valley grows on its own, and it is more
/// than the wastes outside have offered since the Scribe came down from the
/// mountain. It is a bridge to the first harvest, not a living.
const FORAGE_PLACEMENTS: [(f32, f32, &str); 12] = [
    (-1_760.0, 690.0, "world/forage_fungus.png"),
    (-1_240.0, -560.0, "world/forage_greens.png"),
    (-980.0, 980.0, "world/forage_agave.png"),
    (-820.0, -1_180.0, "world/forage_fungus.png"),
    (-620.0, 700.0, "world/forage_greens.png"),
    (-180.0, 900.0, "world/forage_agave.png"),
    (260.0, -980.0, "world/forage_fungus.png"),
    (620.0, 640.0, "world/forage_greens.png"),
    (1_020.0, -420.0, "world/forage_agave.png"),
    (1_380.0, 780.0, "world/forage_fungus.png"),
    (1_720.0, -1_140.0, "world/forage_greens.png"),
    (2_040.0, 460.0, "world/forage_agave.png"),
];
const FORAGE_SIZE: Vec2 = Vec2::new(48.0, 48.0);
/// A day's eating, near enough, and never more than that.
const FORAGE_RATIONS: u16 = 1;
/// The one sack of seed grain in the valley, kept dry on a tool-shed shelf. Any
/// seed after this has to be grown, traded for, or given.
const SHED_SEED_STORE: u16 = 3;
const SHED_SEED_STORE_ID: &str = "seed-shelf";

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

/// Everything the world starts out knowing about itself. Split out from
/// `run_game` because the registration list is long enough on its own to bury
/// the schedule that follows it.
fn install_resources(app: &mut App) -> &mut App {
    app.insert_resource(ClearColor(Color::srgb(0.08, 0.09, 0.08)))
        .insert_resource(UiScale(DEVELOPMENT_PRESENTATION_SCALE))
        .insert_resource(Journal::at_arrival())
        .insert_resource(InterpretInbox::default())
        .init_resource::<Clock>()
        .init_resource::<Chance>()
        .init_resource::<Visitors>()
        .init_resource::<Collection>()
        .init_resource::<Readings>()
        .init_resource::<Salvaged>()
        .insert_resource(initial_world_location())
        .insert_resource(MotelAccess::default())
        .insert_resource(Progression::default())
        .insert_resource(Garden::default())
        .init_resource::<GardenBeds>()
        .insert_resource(ExteriorReturn::default())
        .init_resource::<NarrativePopup>()
        .init_resource::<Portfolio>()
        .init_resource::<DoorwayAttempt>()
        .init_resource::<DoorBumpLatch>()
        .init_resource::<terrain::TerrainDebugOverlay>()
        .init_resource::<InteriorState>()
        .init_resource::<SceneVisualsDirty>()
        // What the waystation spends every night it stands. See `mod upkeep`.
        .init_resource::<upkeep::Upkeep>()
        // Off unless `WAYSTATION_VISITORS` says otherwise, in which case a
        // traveller is already coming down the road. See `mod rehearsal`.
        .insert_resource(rehearsal::Rehearsal::from_environment())
        .init_resource::<rehearsal::Rehearsed>()
}

fn run_game() {
    let mut app = App::new();
    install_resources(&mut app)
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
            (
                load_story,
                // After the save, so a rehearsal overrides one; before the
                // world, so the hearth it lights is built already lit.
                rehearsal::warm_the_waystation,
                setup_world,
                load_ui_fonts,
                setup_ui,
            )
                .chain(),
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
                (grow_garden, sync_garden_plots).chain(),
                (
                    chance::stir_chance,
                    advance_clock,
                    reconcile_scene_visuals,
                    sync_daylight,
                )
                    .chain(),
                (
                    npc_art::compose_visitor_art,
                    rehearsal::summon_visitors,
                    run_visits,
                    retire_visitors,
                )
                    .chain(),
                update_nearby_interaction,
                (handle_portfolio_input, handle_tool_hotkeys).chain(),
                handle_visit_input,
                poll_interpretation,
                sync_ui,
                (sync_narrative_popup_ui, sync_portfolio_ui).chain(),
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
    /// The walk row inside a full 54-row LPC action sheet, which is what the
    /// Scribe's own art is.
    const fn walk_row(self) -> usize {
        match self {
            Self::Up => 8,
            Self::Left => 9,
            Self::Down => 10,
            Self::Right => 11,
        }
    }

    /// The same four rows in a visitor sheet, which carries the walk cycle and
    /// nothing else. Composing a traveller means composing every layer, so the
    /// fifty rows nobody ever sees are not worth the pixels.
    const fn visitor_walk_row(self) -> usize {
        match self {
            Self::Up => 0,
            Self::Left => 1,
            Self::Down => 2,
            Self::Right => 3,
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

/// The work cycles the Scribe has bodies for. The swung tools ride the LPC
/// slash rows in oversized frames; the long-handled tools ride the thrust rows
/// at the body's own frame size, so an animation has to carry its own geometry
/// rather than assume one atlas shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ToolWorkAnimation {
    Hammer,
    Axe,
    Hoe,
    WateringCan,
    Shovel,
}

impl ToolWorkAnimation {
    const ALL: [Self; 5] = [
        Self::Hammer,
        Self::Axe,
        Self::Hoe,
        Self::WateringCan,
        Self::Shovel,
    ];

    const fn art_name(self) -> &'static str {
        match self {
            Self::Hammer => "world/scribe-hammer.png",
            Self::Axe => "world/scribe-axe.png",
            Self::Hoe => "world/scribe-hoe.png",
            Self::WateringCan => "world/scribe-watering-can.png",
            Self::Shovel => "world/scribe-shovel.png",
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Hammer => 0,
            Self::Axe => 1,
            Self::Hoe => 2,
            Self::WateringCan => 3,
            Self::Shovel => 4,
        }
    }

    const fn swung(self) -> bool {
        matches!(self, Self::Hammer | Self::Axe)
    }

    const fn frame_size(self) -> u32 {
        if self.swung() {
            SCRIBE_TOOL_FRAME_SIZE
        } else {
            SCRIBE_THRUST_FRAME_SIZE
        }
    }

    const fn columns(self) -> u32 {
        if self.swung() {
            SCRIBE_TOOL_COLUMNS
        } else {
            SCRIBE_THRUST_COLUMNS
        }
    }

    /// The cycle a task's own tools call for, so a new tool does not also need a
    /// new branch at every place work happens.
    fn for_task(task: &progression::TaskSpec) -> Option<Self> {
        task.tools.iter().find_map(|tool| match tool {
            // No pick layer is drawn yet; the hammer swing is the nearest body.
            ToolId::Hammer | ToolId::Pickaxe => Some(Self::Hammer),
            ToolId::Hatchet => Some(Self::Axe),
            ToolId::Hoe => Some(Self::Hoe),
            ToolId::WateringCan => Some(Self::WateringCan),
            ToolId::Shovel => Some(Self::Shovel),
            ToolId::Trowel | ToolId::Ladder => None,
        })
    }
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
    /// One entry per `ToolWorkAnimation`, in `ToolWorkAnimation::index` order.
    work_cycles: Vec<(Handle<Image>, Handle<TextureAtlasLayout>)>,
}

impl PlayerArt {
    fn work_cycle(&self, kind: ToolWorkAnimation) -> &(Handle<Image>, Handle<TextureAtlasLayout>) {
        &self.work_cycles[kind.index()]
    }
}

#[derive(Component)]
struct MainCamera;

/// One body of an arriving party. The index selects its authored sheet and the
/// offset that keeps a pair from standing inside one another.
#[derive(Component)]
struct VisitorBody {
    index: usize,
}

/// Visitors walk slower than the Scribe. They have come a long way and they are
/// not sure about this.
const VISITOR_SPEED: f32 = 96.0;

/// Where the old road reaches the edge of the valley floor. Parties enter and
/// leave here, so they are seen coming rather than appearing in the court.
const fn visitor_road_entry() -> Vec2 {
    Vec2::new(-1_260.0, -300.0)
}

/// Where somebody stands when they have come as close as they dare: out in the
/// open in front of the office, within sight of the door and of the way back.
const fn visitor_waiting_spot() -> Vec2 {
    Vec2::new(-540.0, -200.0)
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum InteractableKind {
    Sign,
    Kindling,
    Log,
    Forage,
    SeedStore,
    Hearth,
    Plank,
    Tool,
    Desk,
    BibleNightstand,
    Visitor,
    Bed,
    Salvage,
    MotelDoor,
    InteriorExit,
    InteriorRepairable,
    ExteriorRepairable,
    Tree,
    Sawbuck,
    StoneOutcrop,
    GardenPlot,
    RainCistern,
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

#[derive(Component)]
struct StoneOutcrop {
    id: String,
    footprint: ExteriorRect,
    art: ExteriorRect,
}

/// One bed of ground the Scribe is trying to bring back. Unlike every other
/// station, what it asks for changes as it goes, so the entity carries only its
/// identity and reads its state out of the saved `Garden`.
/// One authored parking bay. Its two content-owned faces and the task that
/// turns one into the other come from the repair pair; everything after that is
/// the garden's.
#[derive(Component)]
struct GardenPlot {
    id: String,
    /// The art currently on the sprite, so the reconciler only reloads on a
    /// state change rather than every frame.
    art: String,
    paved: String,
    broken: String,
    break_task: progression::TaskSpec,
}

impl GardenPlot {
    /// What this bay is waiting for. Levering the slab up is authored on the
    /// repair pair; every state after that belongs to the garden.
    fn work_task(&self, stage: PlotStage) -> Option<progression::TaskSpec> {
        if stage.is_paved() {
            return Some(self.break_task.clone());
        }
        stage.task()
    }

    fn art_for(&self, stage: PlotStage, nearly_ripe: bool) -> &str {
        if let Some(grown) = stage.grown_art(nearly_ripe) {
            return grown;
        }
        if stage.is_paved() {
            &self.paved
        } else {
            &self.broken
        }
    }
}

/// The motel's rain butt. Two states, tracked in the same saved scene-state map
/// the repair pairs use, so a repaired barrel stays repaired across a save.
#[derive(Component)]
struct RainCistern {
    art: &'static str,
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

/// How many bays the authored lot laid down, so the status panel can say
/// "3 of 9" without loading the scene again.
#[derive(Resource, Default)]
struct GardenBeds(usize);

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

/// Which authored interaction rectangle an entity came from, so emptying it can
/// be written into save state under the same key the scene uses.
#[derive(Component)]
struct SceneInteractionId(String);

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

/// Set when something other than an interaction moved a scene element's state,
/// so `reconcile_scene_visuals` can stay asleep the rest of the time. Ordinary
/// change detection is no use here: half the game holds `InteriorState` mutably
/// and marks it changed without touching it.
#[derive(Resource, Default)]
struct SceneVisualsDirty(bool);

#[derive(SystemParam)]
struct InteractionResources<'w> {
    interior_state: ResMut<'w, InteriorState>,
    motel_access: ResMut<'w, MotelAccess>,
    progression: ResMut<'w, Progression>,
    garden: ResMut<'w, Garden>,
    visitors: ResMut<'w, Visitors>,
    readings: ResMut<'w, Readings>,
    salvaged: ResMut<'w, Salvaged>,
    chance: ResMut<'w, Chance>,
    clock: ResMut<'w, Clock>,
    upkeep: ResMut<'w, upkeep::Upkeep>,
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
    garden_plots: Query<'w, 's, &'static GardenPlot>,
    interaction_labels: Query<'w, 's, &'static AuthoredInteractionLabel>,
    interaction_ids: Query<'w, 's, &'static SceneInteractionId>,
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
struct UiKnowledge<'w, 's> {
    interior_state: Res<'w, InteriorState>,
    motel_access: Res<'w, MotelAccess>,
    garden: Res<'w, Garden>,
    beds: Res<'w, GardenBeds>,
    clock: Res<'w, Clock>,
    visitors: Res<'w, Visitors>,
    collection: Res<'w, Collection>,
    readings: Res<'w, Readings>,
    upkeep: Res<'w, upkeep::Upkeep>,
    garden_plots: Query<'w, 's, &'static GardenPlot>,
    rain_cisterns: Query<'w, 's, &'static RainCistern, Without<GardenPlot>>,
}

/// Every widget the full-screen narrative overlay owns. Bundled because a Bevy
/// system takes at most sixteen parameters and `sync_ui` writes to all of them.
#[derive(SystemParam)]
// Each text node excludes every other text marker so Bevy can prove the borrows
// are disjoint; that is what makes these signatures long, not the logic.
#[allow(clippy::type_complexity)]
struct OverlayWidgets<'w, 's> {
    root: Query<'w, 's, &'static mut Visibility, With<OverlayRoot>>,
    title: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<OverlayTitle>,
            Without<OverlayBody>,
            Without<StatusText>,
            Without<PromptText>,
            Without<ProgressText>,
        ),
    >,
    body: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<OverlayBody>,
            Without<OverlayTitle>,
            Without<StatusText>,
            Without<PromptText>,
            Without<ProgressText>,
        ),
    >,
    // The slot is emptied out of the layout rather than merely hidden. A hidden
    // node still holds its place, which on the screens carrying no art would
    // leave a card-shaped gap beside the words.
    card_art: Query<'w, 's, (&'static mut ImageNode, &'static mut Node), With<CardArt>>,
    prompt_panel: Query<'w, 's, &'static mut Visibility, (With<PromptPanel>, Without<OverlayRoot>)>,
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

/// The full-screen wash that carries the time of day.
#[derive(Component)]
struct NightTint;

#[derive(Component)]
struct StatusText;

#[derive(Component)]
struct PromptText;

/// The parchment the nearby-interaction prompt sits on. It hides during a
/// conversation, where "E — go and speak to them" is both wrong and in the way.
#[derive(Component)]
struct PromptPanel;

#[derive(Component)]
struct OverlayRoot;

#[derive(Component)]
struct OverlayTitle;

#[derive(Component)]
struct OverlayBody;

#[derive(Component)]
struct ProgressText;

#[derive(Component)]
struct CardArt;

/// `scripts/build-print-cards.py` composes every card at 512×768. Any node
/// drawing one has to carry that shape or the block comes out squashed.
const CARD_ASPECT: f32 = 512.0 / 768.0;

/// What a card slot holds before it has been told which print to draw. It is
/// never on screen: both slots start with their `Display` off.
const CARD_PLACEHOLDER_PATH: &str = "card/illustration_1_1.png";

/// How much of the overlay a card takes across. The web shell stretches the
/// canvas to whatever window it is given, so the space these panels are laid
/// out in is not the authored 960×540 and a fixed pixel width would overrun a
/// short window. Everything here is a share of the screen instead.
const OVERLAY_CARD_SHARE: f32 = 28.0;

/// How much of the screen the open folio covers, and how much of it the words
/// beside the leaf take. The card fills the height that leaves and its width
/// follows from the shape, so a short window gets a smaller card rather than
/// one running off the bottom.
const FOLIO_HEIGHT_SHARE: f32 = 90.0;
const FOLIO_WIDTH_SHARE: f32 = 88.0;
const FOLIO_TEXT_SHARE: f32 = 56.0;

#[derive(Component)]
struct PortfolioRoot;

#[derive(Component)]
struct PortfolioArt;

#[derive(Component)]
struct PortfolioCaption;

#[derive(Component)]
struct PortfolioTally;

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

/// One card the game holds up in front of the player. Authored beats carry
/// static text; a passage read from the book or a thing pulled out of a drawer
/// is chosen at play time and carries its own.
/// The Scribe's own folio of block-prints, openable at any hour and anywhere.
/// The prints already carried off by somebody are still in it: the block is
/// still in the shed and the hands still remember cutting it.
#[derive(Resource, Default, Debug)]
struct Portfolio {
    open: bool,
    index: usize,
}

impl Portfolio {
    const fn is_open(&self) -> bool {
        self.open
    }

    /// A folio that grew while it was shut still opens at a leaf that exists.
    fn open_at_a_real_leaf(&mut self, cut: usize) {
        self.open = true;
        self.index = self.index.min(cut.saturating_sub(1));
    }

    /// Turning past the last leaf comes back to the first, so no direction is
    /// ever a dead end and a folio of one still answers the key.
    const fn turn_forward(&mut self, cut: usize) {
        if cut > 0 {
            self.index = (self.index + 1) % cut;
        }
    }

    const fn turn_back(&mut self, cut: usize) {
        if cut > 0 {
            self.index = (self.index + cut - 1) % cut;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum NarrativeCard {
    Item(DiscoveredItem),
    Thought(StoryBeat),
    /// A page of the book, read and put back.
    Passage {
        title: String,
        body: String,
    },
    /// Whatever was in the drawer.
    Salvage {
        title: String,
        body: String,
    },
}

impl NarrativeCard {
    fn title(&self) -> &str {
        match self {
            Self::Item(item) => item.title(),
            Self::Thought(beat) => beat.title(),
            Self::Passage { title, .. } | Self::Salvage { title, .. } => title,
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::Item(item) => item.description(),
            Self::Thought(beat) => beat.description(),
            Self::Passage { body, .. } | Self::Salvage { body, .. } => body,
        }
    }

    const fn image_path(&self) -> Option<&'static str> {
        match self {
            Self::Item(item) => Some(item.image_path()),
            Self::Passage { .. } => Some(BIBLE_ICON_PATH),
            Self::Thought(_) | Self::Salvage { .. } => None,
        }
    }

    const fn dismiss_label(&self) -> &'static str {
        match self {
            Self::Item(DiscoveredItem::GideonBible) => "E / SPACE — leave it safe here",
            Self::Passage { .. } => "E / SPACE — close the book",
            Self::Thought(_) | Self::Salvage { .. } => "E / SPACE — continue",
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
        if self.current.as_ref() == Some(&card) || self.queue.contains(&card) {
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

/// The single line of prose the game is allowed to put on screen unasked.
///
/// It reports what just happened and never what to do next. There is no
/// objective here and no next step, because working out what a ruin needs is the
/// game. Anything that looks like an instruction belongs on the thing it is
/// about, shown when the player walks up to it and asks.
#[derive(Resource, Default)]
struct Journal {
    notice: Option<String>,
}

impl Journal {
    /// The one line already on screen when the game starts.
    fn at_arrival() -> Self {
        Self {
            notice: Some(ARRIVAL_NOTICE.to_owned()),
        }
    }

    fn say(&mut self, notice: impl Into<String>) {
        self.notice = Some(notice.into());
    }
}

/// What the arrival looks like before anybody has told the player anything.
const ARRIVAL_NOTICE: &str =
    "The storm has followed you for two days. Then, below the ridge: stone walls.";

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Debug, Serialize, Deserialize)]
struct SaveData {
    version: u8,
    #[serde(default)]
    interior_states: HashMap<String, String>,
    #[serde(default)]
    motel_keys_found: bool,
    #[serde(default)]
    progression: Progression,
    #[serde(default)]
    garden: Garden,
    #[serde(default)]
    clock: Clock,
    #[serde(default)]
    nights_of_smoke: u32,
    #[serde(default)]
    visits_received: u32,
    #[serde(default)]
    prints_made: Vec<String>,
    #[serde(default)]
    prints_given: Vec<String>,
    #[serde(default)]
    print_tier: cards::Tier,
    #[serde(default)]
    passages_read: Vec<String>,
    #[serde(default)]
    dwelling_on: Option<String>,
    #[serde(default)]
    salvaged: Vec<String>,
    #[serde(default)]
    upkeep: upkeep::Upkeep,
}

/// What the world knows about itself, gathered for saving and restoring. The
/// bundle exists because eleven separate resources will not fit in a system's
/// parameter list beside everything else a save needs.
#[derive(SystemParam)]
struct WorldMemory<'w> {
    interior_state: Res<'w, InteriorState>,
    motel_access: Res<'w, MotelAccess>,
    progression: Res<'w, Progression>,
    garden: Res<'w, Garden>,
    clock: Res<'w, Clock>,
    visitors: Res<'w, Visitors>,
    collection: Res<'w, Collection>,
    readings: Res<'w, Readings>,
    salvaged: Res<'w, Salvaged>,
    upkeep: Res<'w, upkeep::Upkeep>,
}

impl SaveData {
    #[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
    #[allow(clippy::too_many_arguments)]
    fn capture(
        interior_state: &InteriorState,
        motel_access: &MotelAccess,
        progression: &Progression,
        garden: &Garden,
        clock: Clock,
        visitors: &Visitors,
        collection: &Collection,
        readings: &Readings,
        salvaged: &Salvaged,
        upkeep: &upkeep::Upkeep,
    ) -> Self {
        let (prints_made, prints_given, print_tier) = collection.saved();
        let (passages_read, dwelling_on) = readings.saved();
        Self {
            version: 9,
            interior_states: interior_state.0.clone(),
            motel_keys_found: motel_access.keys_found,
            progression: progression.clone(),
            garden: garden.clone(),
            clock,
            nights_of_smoke: visitors.nights_of_smoke,
            visits_received: visitors.visits_received,
            prints_made,
            prints_given,
            print_tier,
            passages_read,
            dwelling_on,
            salvaged: salvaged.seen().to_vec(),
            upkeep: upkeep.clone(),
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn from_memory(memory: &WorldMemory) -> Self {
        Self::capture(
            &memory.interior_state,
            &memory.motel_access,
            &memory.progression,
            &memory.garden,
            *memory.clock,
            &memory.visitors,
            &memory.collection,
            &memory.readings,
            &memory.salvaged,
            &memory.upkeep,
        )
    }
}

type InboxValue = Option<Result<InterpretResponse, String>>;

#[derive(Resource, Clone, Default)]
struct InterpretInbox(Arc<Mutex<InboxValue>>);

#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
fn setup_world(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    location: Res<WorldLocation>,
    mut interior_state: ResMut<InteriorState>,
    mut popup: ResMut<NarrativePopup>,
    mut progression: ResMut<Progression>,
    garden: Res<Garden>,
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
    let work_cycle_images = ToolWorkAnimation::ALL
        .map(|kind| asset_server.load::<Image>(kind.art_name()))
        .to_vec();
    let motel = interior::MotelExteriorMap::load();
    let parking = interior::MotelParkingMap::load();
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

    // The sign is the first built thing the road shows the Scribe. It stands on
    // its posts the way a tree stands on its trunk: sorted by where it meets the
    // ground, solid at the base, and no place for a fallen log to land.
    exterior_obstacles
        .solid_footprints
        .push(station_footprint(MOTEL_SIGN_POSITION, MOTEL_SIGN_SIZE));
    exterior_obstacles
        .prop_exclusions
        .push(ExteriorRect::new(MOTEL_SIGN_POSITION, MOTEL_SIGN_SIZE));
    commands.spawn((
        Sprite {
            image: asset_server.load("world/way_station_sign.png"),
            custom_size: Some(MOTEL_SIGN_SIZE),
            ..default()
        },
        Transform::from_xyz(
            MOTEL_SIGN_POSITION.x,
            MOTEL_SIGN_POSITION.y,
            exterior_depth(MOTEL_SIGN_POSITION.y - MOTEL_SIGN_SIZE.y / 2.0),
        ),
        ExteriorYSort {
            ground_offset_y: -MOTEL_SIGN_SIZE.y / 2.0,
            depth_bias: 0.0,
        },
        Interactable {
            kind: InteractableKind::Sign,
            consumed: false,
        },
    ));
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

    // Standing work stations, not one-time props: each outcrop carries stone for
    // the masonry the motel is full of. The sawbuck is the other one, and it
    // stands indoors — see `content/interiors/tool-shed-interior.json`.
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

    // The beds are the motel's parking bays, laid out in `content/buildings/
    // motel-parking.json` so the lot can be edited rather than recompiled. They
    // are flat ground art the Scribe walks over, so they skip `spawn_building`
    // and its layer caches, collision, and depth sorting entirely.
    commands.insert_resource(GardenBeds(parking.mutable_elements().len()));
    for element in parking.mutable_elements() {
        let paved = element
            .states
            .get("damaged")
            .and_then(|visual| visual.image_path.clone());
        let broken = element
            .states
            .get("repaired")
            .and_then(|visual| visual.image_path.clone());
        let (Some(paved), Some(broken)) = (paved, broken) else {
            continue;
        };
        let size = element
            .states
            .get("damaged")
            .map_or(GARDEN_PLOT_SIZE, |visual| visual.size);
        let position = parking.element_center(element, size);
        pickup_bounds.push(ExteriorRect::new(position, size));
        exterior_obstacles
            .prop_exclusions
            .push(ExteriorRect::new(position, size));
        let stage = garden.stage(&element.id);
        let art = stage
            .grown_art(garden.nearly_ripe(&element.id))
            .map_or_else(
                || {
                    if stage.is_paved() {
                        paved.clone()
                    } else {
                        broken.clone()
                    }
                },
                ToOwned::to_owned,
            );
        commands.spawn((
            Sprite {
                image: asset_server.load(&art),
                custom_size: Some(size),
                flip_x: element.flip_x,
                flip_y: element.flip_y,
                ..default()
            },
            Transform::from_xyz(position.x, position.y, GARDEN_PLOT_DEPTH),
            Interactable {
                kind: InteractableKind::GardenPlot,
                consumed: false,
            },
            GardenPlot {
                id: element.id.clone(),
                art,
                paved,
                broken,
                // What it takes to lever this slab up is authored on the pair.
                break_task: element.task.clone(),
            },
        ));
    }

    // The motel's own rain butt, staved in. It is a repair before it is a water
    // source, so the garden never gets its water for nothing.
    let cistern_position = safe_pickup_position(
        &world_grid,
        &motel,
        &tool_shed,
        &exterior_obstacles,
        &pickup_bounds,
        RAIN_CISTERN_POSITION,
        RAIN_CISTERN_SIZE,
    );
    pickup_bounds.push(ExteriorRect::new(cistern_position, RAIN_CISTERN_SIZE));
    let cistern_repaired = cistern_holds_water(&interior_state);
    spawn_worked_station(
        &mut commands,
        asset_server.load(cistern_art(cistern_repaired)),
        InteractableKind::RainCistern,
        progression::TaskSpec::for_drawing_water(),
        cistern_position,
        RAIN_CISTERN_SIZE,
        &mut exterior_obstacles,
    )
    .insert(RainCistern {
        art: cistern_art(cistern_repaired),
    })
    // Like a bed, what it asks for depends on whether it holds water yet.
    .remove::<TaskTarget>();

    // What the valley grows on its own. These are the only free food in the
    // game, and there are twelve of them.
    for (index, (x, y, art)) in FORAGE_PLACEMENTS.into_iter().enumerate() {
        spawn_safe_world_pickup(
            &mut commands,
            &progression,
            &world_grid,
            &motel,
            &tool_shed,
            &exterior_obstacles,
            &mut pickup_bounds,
            format!("forage-{index:02}"),
            InteractableKind::Forage,
            Vec2::new(x, y),
            FORAGE_SIZE,
            Sprite::from_image(asset_server.load(art)),
            PickupReward::Supply(SupplyId::Ration, FORAGE_RATIONS),
        );
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
    let work_cycles = ToolWorkAnimation::ALL
        .into_iter()
        .zip(work_cycle_images)
        .map(|(kind, image)| {
            (
                image,
                texture_atlas_layouts.add(TextureAtlasLayout::from_grid(
                    UVec2::splat(kind.frame_size()),
                    kind.columns(),
                    SCRIBE_TOOL_ROWS,
                    None,
                    None,
                )),
            )
        })
        .collect();
    commands.insert_resource(PlayerArt {
        walk_image: scribe.clone(),
        walk_layout: scribe_layout.clone(),
        work_cycles,
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

/// The authored interactions that bring their own art rather than sitting over
/// scenery the room already draws: the seed sack, which is the only thing on its
/// shelf and goes when the shelf is emptied, and the sawbuck, which fills the
/// rectangle the scene gives it — so moving or resizing the bench is a content
/// edit and not a code one.
fn spawn_scene_interaction_art(
    commands: &mut Commands,
    asset_server: &AssetServer,
    kind: InteractableKind,
    interaction: &interior::SceneInteraction,
    previously_discovered: bool,
) -> Option<Entity> {
    let image = match kind {
        InteractableKind::SeedStore => "world/seed_sack.png",
        InteractableKind::Sawbuck => "world/sawbuck.png",
        _ => return None,
    };
    let entity = spawn_interactable_sprite(
        commands,
        kind,
        interaction.center,
        Sprite {
            image: asset_server.load(image),
            custom_size: Some(interaction.size),
            ..default()
        },
    );
    commands.entity(entity).insert(Transform::from_xyz(
        interaction.center.x,
        interaction.center.y,
        INTERIOR_PLAYER_DEPTH - 1.0,
    ));
    if kind == InteractableKind::SeedStore {
        commands.entity(entity).insert(if previously_discovered {
            Visibility::Hidden
        } else {
            Visibility::Visible
        });
    }
    if kind == InteractableKind::Sawbuck {
        let task = progression::TaskSpec::for_milling();
        commands.entity(entity).insert(TaskTarget {
            action: task.action,
            requirements: task.requirements_text(),
        });
    }
    Some(entity)
}

/// One authored interaction rectangle: the Bible's nightstand, the seed shelf,
/// the sawbuck. The Bible sits over furniture that is already drawn, so its
/// rectangle stays invisible; anything with art of its own goes through
/// `spawn_scene_interaction_art`.
fn spawn_scene_interaction(
    commands: &mut Commands,
    asset_server: &AssetServer,
    map: &interior::InteriorMap,
    interior_state: &InteriorState,
    interaction: &interior::SceneInteraction,
) {
    let previously_discovered = interior_state
        .0
        .get(&format!("{}/{}", map.id(), interaction.id))
        .is_some_and(|state| state == DISCOVERY_FOUND_STATE);
    let kind = match (interaction.kind, interaction.discovery) {
        (interior::SceneInteractionKind::Search, interior::SceneDiscovery::GideonBible) => {
            InteractableKind::BibleNightstand
        }
        (interior::SceneInteractionKind::Search, interior::SceneDiscovery::SeedStore) => {
            InteractableKind::SeedStore
        }
        (interior::SceneInteractionKind::Search, interior::SceneDiscovery::Salvage) => {
            InteractableKind::Salvage
        }
        (interior::SceneInteractionKind::Rest, interior::SceneDiscovery::Bed) => {
            InteractableKind::Bed
        }
        (interior::SceneInteractionKind::Work, interior::SceneDiscovery::Sawbuck) => {
            InteractableKind::Sawbuck
        }
        (kind, discovery) => panic!(
            "{}/{} is authored as {kind:?} yielding {discovery:?}, which the game has no pairing for",
            map.id(),
            interaction.id
        ),
    };
    // A hiding place that has already been turned out has nothing more in it,
    // and offering it again would be the game lying to the player.
    if kind == InteractableKind::Salvage && previously_discovered {
        return;
    }
    let drawn = spawn_scene_interaction_art(
        commands,
        asset_server,
        kind,
        interaction,
        previously_discovered,
    );
    let entity = drawn.unwrap_or_else(|| {
        spawn_interactable(
            commands,
            kind,
            interaction.center,
            interaction.size,
            Color::NONE,
        )
    });
    commands.entity(entity).insert((
        interior::InteriorSceneEntity,
        SceneInteractionId(format!("{}/{}", map.id(), interaction.id)),
        AuthoredInteractionLabel(match (kind, previously_discovered) {
            (InteractableKind::BibleNightstand, true) => {
                "nightstand where the little book rests".to_owned()
            }
            (InteractableKind::SeedStore, true) => "empty seed shelf".to_owned(),
            _ => interaction.label.clone(),
        }),
        Interactable {
            kind,
            // Discovery records knowledge but does not remove the object: the
            // Scribe deliberately leaves the Bible here to revisit.
            consumed: false,
        },
    ));
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
        spawn_scene_interaction(commands, asset_server, map, interior_state, interaction);
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
#[allow(clippy::too_many_arguments)]
fn load_story(
    mut journal: ResMut<Journal>,
    mut interior_state: ResMut<InteriorState>,
    mut motel_access: ResMut<MotelAccess>,
    mut progression: ResMut<Progression>,
    mut garden: ResMut<Garden>,
    mut clock: ResMut<Clock>,
    mut visitors: ResMut<Visitors>,
    mut collection: ResMut<Collection>,
    mut readings: ResMut<Readings>,
    mut salvaged: ResMut<Salvaged>,
    mut upkeep: ResMut<upkeep::Upkeep>,
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
    if !matches!(save.version, 1..=9) {
        return;
    }
    interior_state.0 = save.interior_states;
    motel_access.keys_found = save.motel_keys_found;
    *progression = save.progression;
    // Saves older than the garden simply have none; every plot reads as ash.
    *garden = save.garden;
    // Saves older than the clock start their next morning rather than resuming
    // an hour they were never written with.
    *clock = save.clock;
    visitors.nights_of_smoke = save.nights_of_smoke;
    visitors.visits_received = save.visits_received;
    collection.restore(save.prints_made, save.prints_given, save.print_tier);
    readings.restore(save.passages_read, save.dwelling_on);
    salvaged.restore(save.salvaged);
    // Saves older than the upkeep have an empty pot and a cold woodpile, which
    // is exactly what a waystation that was never charged rent should inherit.
    *upkeep = save.upkeep;
    journal.say("The old trail returns to memory.");
}

fn bible_found(interior_state: &InteriorState) -> bool {
    interior_state
        .0
        .get(BIBLE_STATE_KEY)
        .is_some_and(|state| state == DISCOVERY_FOUND_STATE)
}

fn record_bible_discovery(interior_state: &mut InteriorState) {
    interior_state
        .0
        .insert(BIBLE_STATE_KEY.to_owned(), DISCOVERY_FOUND_STATE.to_owned());
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
fn save_story(memory: WorldMemory) {
    // The clock changes every frame, so it is deliberately not a trigger: a save
    // rides along with something the player actually did.
    if !memory.interior_state.is_changed()
        && !memory.motel_access.is_changed()
        && !memory.progression.is_changed()
        && !memory.garden.is_changed()
        && !memory.visitors.is_changed()
        && !memory.collection.is_changed()
        && !memory.readings.is_changed()
        && !memory.salvaged.is_changed()
    {
        return;
    }
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return;
    };
    if let Ok(raw) = serde_json::to_string(&SaveData::from_memory(&memory)) {
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
    portfolio: Res<Portfolio>,
    environment: ToolDropEnvironment,
    mut progression: ResMut<Progression>,
    mut journal: ResMut<Journal>,
    player: Query<(&Transform, &PlayerAnimation), With<Player>>,
) {
    if popup.is_open() || portfolio.is_open() {
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        journal.notice = progression.cycle_equipped_tool().map(|tool| {
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
        journal.notice = Some("You are not carrying a portable tool to put down.".to_owned());
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
    journal.notice = tool.map(|tool| {
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
    // Behind every other panel, so dusk falls on the valley and not on the
    // writing the player needs to read in it.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::NONE),
        GlobalZIndex(-1),
        NightTint,
    ));
    let parchment = BackgroundColor(Color::srgba(0.72, 0.63, 0.45, 0.95));
    let parchment_edge = BorderColor::all(Color::srgb(0.28, 0.20, 0.12));
    let ink = TextColor(Color::srgb(0.16, 0.11, 0.07));
    let quiet_ink = TextColor(Color::srgb(0.25, 0.18, 0.11));
    // Every parchment panel puts its words on a child node rather than on the
    // frame itself. A node that is both the box and the text lays the text out
    // against its border rather than inside its padding, which reads as a line
    // touching the frame on the left and, once a line is justified right, as one
    // walking off the edge of the screen. The child is told to fill the width it
    // is given, so it wraps where the padding ends and a narrow window narrows
    // the writing instead of spilling it.
    let writing = Node {
        width: Val::Percent(100.0),
        ..default()
    };
    commands
        .spawn((
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
        .with_children(|panel| {
            panel
                .spawn((Text::new("📜  "), fonts.emoji(10.0), ink, writing.clone()))
                .with_child((TextSpan::new(""), fonts.roman(10.0), ink, StatusText));
        });
    commands
        .spawn((
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
        ))
        .with_child((
            Text::new(""),
            fonts.roman(8.0),
            quiet_ink,
            TextLayout::new_with_justify(Justify::Right),
            writing.clone(),
            ProgressText,
        ));
    commands
        .spawn((
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
            PromptPanel,
        ))
        .with_children(|panel| {
            panel
                .spawn((
                    Text::new("☞  "),
                    fonts.emoji(10.0),
                    ink,
                    TextLayout::new_with_justify(Justify::Center),
                    writing,
                ))
                .with_child((TextSpan::new(""), fonts.roman(10.0), ink, PromptText));
        });

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(8.0),
                right: Val::Percent(8.0),
                top: Val::Percent(3.0),
                bottom: Val::Percent(3.0),
                padding: UiRect::all(Val::Px(12.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(10.0),
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
                fonts.roman(22.0),
                TextColor(Color::srgb(0.95, 0.79, 0.39)),
                OverlayTitle,
            ));
            // A print is portrait and this overlay is landscape, so the block
            // sits beside the words rather than above them. Stacked, the only
            // room left for it is a letterbox slot, and a card squeezed into
            // one is a card nobody can read.
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Px(16.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        ImageNode::new(asset_server.load(CARD_PLACEHOLDER_PATH)),
                        Node {
                            display: Display::None,
                            width: Val::Percent(OVERLAY_CARD_SHARE),
                            aspect_ratio: Some(CARD_ASPECT),
                            flex_shrink: 0.0,
                            ..default()
                        },
                        CardArt,
                    ));
                    row.spawn(Node {
                        flex_grow: 1.0,
                        flex_basis: Val::Px(0.0),
                        min_width: Val::Px(0.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        row_gap: Val::Px(10.0),
                        ..default()
                    })
                    .with_children(|column| {
                        // Text wraps to the width its own node is given, not to
                        // whatever its parent happens to be, so each block is
                        // told to take the column and no more.
                        column.spawn((
                            Text::new(""),
                            fonts.roman(14.0),
                            TextColor(Color::srgb(0.93, 0.90, 0.80)),
                            Node {
                                max_width: Val::Percent(100.0),
                                ..default()
                            },
                            OverlayBody,
                        ));
                    });
                });
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
            BackgroundColor(Color::srgba(0.04, 0.03, 0.02, 0.74)),
            // Global, because what the folio has to cover — the status corner,
            // the prompt strip — are separate roots rather than siblings.
            GlobalZIndex(190),
            Visibility::Hidden,
            PortfolioRoot,
        ))
        .with_children(|backdrop| {
            backdrop
                .spawn((
                    Node {
                        width: Val::Percent(FOLIO_WIDTH_SHARE),
                        height: Val::Percent(FOLIO_HEIGHT_SHARE),
                        padding: UiRect::all(Val::Px(14.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        column_gap: Val::Px(16.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.07, 0.06, 0.97)),
                    BorderColor::all(Color::srgb(0.58, 0.48, 0.28)),
                    Outline::new(Val::Px(3.0), Val::Px(0.0), Color::srgb(0.58, 0.48, 0.28)),
                ))
                .with_children(|panel| {
                    // A card is taller than it is wide and the window is not, so
                    // the height it can have is settled first and the width
                    // follows from the shape rather than the other way round.
                    panel.spawn((
                        ImageNode::new(asset_server.load(CARD_PLACEHOLDER_PATH)),
                        Node {
                            display: Display::None,
                            height: Val::Percent(100.0),
                            aspect_ratio: Some(CARD_ASPECT),
                            flex_shrink: 0.0,
                            ..default()
                        },
                        PortfolioArt,
                    ));
                    panel
                        .spawn(Node {
                            width: Val::Percent(FOLIO_TEXT_SHARE),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            row_gap: Val::Px(12.0),
                            ..default()
                        })
                        .with_children(|column| {
                            column.spawn((
                                Text::new(""),
                                fonts.roman(14.0),
                                TextColor(Color::srgb(0.93, 0.90, 0.80)),
                                // Text wraps to the width its own node is given,
                                // not to whatever its parent happens to be.
                                Node {
                                    max_width: Val::Percent(100.0),
                                    ..default()
                                },
                                PortfolioCaption,
                            ));
                            column.spawn((
                                Text::new(""),
                                fonts.roman(12.0),
                                TextColor(Color::srgb(0.62, 0.66, 0.61)),
                                Node {
                                    max_width: Val::Percent(100.0),
                                    ..default()
                                },
                                PortfolioTally,
                            ));
                        });
                });
        });
}

fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    visitors: Res<Visitors>,
    popup: Res<NarrativePopup>,
    portfolio: Res<Portfolio>,
    mut environment: MovementEnvironment,
    mut player: Query<(&mut Transform, &mut PlayerAnimation), With<Player>>,
) {
    environment.doorway_attempt.0 = None;
    // Walking away mid-sentence is rude and, worse, leaves a conversation on
    // screen with nobody in front of it. The same goes for the arrow keys while
    // they are turning leaves in the folio.
    if popup.is_open() || portfolio.is_open() || visitor_holds_the_screen(&visitors) {
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
    mut journal: ResMut<Journal>,
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
                journal.notice = Some(format!(
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
        journal.notice = Some(format!(
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
        journal.notice = Some("You step back into the valley air.".to_owned());
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
        let columns = active.kind.columns() as usize;
        let (image, layout) = art.work_cycle(active.kind);
        sprite.image = image.clone();
        sprite.texture_atlas = Some(TextureAtlas {
            layout: layout.clone(),
            index: animation.facing.direction_index() * columns + active.frame,
        });
        animation.timer.tick(time.delta());
        if animation.timer.just_finished() {
            active.frame += 1;
            if active.frame >= columns {
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
    let Some(kind) = ToolWorkAnimation::for_task(task) else {
        return;
    };
    if let Ok(mut animation) = player_animation.single_mut() {
        start_tool_animation(&mut animation, kind);
    }
    if task.tools.contains(&ToolId::Hammer) {
        game_audio::play_hammering(commands, asset_server);
    }
}

/// What the prompt says with nothing underfoot. A broken tool in the pack is
/// the one job the Scribe carries around with them, so it displaces the
/// controls line — and, like every other station, it names what it wants
/// rather than telling the player where to go and get it.
fn carried_tool_prompt(progression: &Progression) -> String {
    progression.carried_broken_tool().map_or_else(
        || {
            "Move: WASD/arrows  ·  E interact  ·  R work  ·  Tab tool  ·  Q drop  ·  P prints"
                .to_owned()
        },
        |(_, tool)| {
            format!(
                "R — repair the broken {}     [{}]",
                tool.label(),
                progression::TaskSpec::for_tool_repair(tool).requirements_text()
            )
        },
    )
}

/// One tool put back into service, wherever it is lying: the requirement
/// check, the swing, the sound, and the line saying what it cost. Shared so a
/// tool in the pack mends by exactly the rules a tool on the shed floor does.
fn mend_tool(
    commands: &mut Commands,
    asset_server: &AssetServer,
    progression: &mut Progression,
    journal: &mut Journal,
    player_animation: &mut Query<&mut PlayerAnimation, With<Player>>,
    id: &str,
    label: &str,
) {
    let Some(record) = progression.tool_record(id).cloned() else {
        return;
    };
    if record.condition == ToolCondition::Serviceable {
        journal.say(format!("The {label} is already in working order."));
        return;
    }
    let task = progression::TaskSpec::for_tool_repair(record.tool);
    let outcome = match progression.attempt(&task) {
        Ok(outcome) => outcome,
        Err(reason) => {
            // The refusal names only what stopped it. The full requirement
            // list belongs on the prompt bar, which is the width of the screen;
            // the notice panel is 180px and clips a line it cannot wrap.
            journal.say(format!("You cannot repair the {label} yet. {reason}"));
            return;
        }
    };
    progression.set_tool_condition(id, ToolCondition::Serviceable);
    if task.tools.contains(&ToolId::Hammer) {
        if let Ok(mut animation) = player_animation.single_mut() {
            start_tool_animation(&mut animation, ToolWorkAnimation::Hammer);
        }
        game_audio::play_hammering(commands, asset_server);
    }
    journal.say(if outcome.new_level > outcome.old_level {
        format!(
            "You put the {label} back into working order. Upkeep rises to level {}!",
            outcome.new_level
        )
    } else {
        format!(
            "You put the {label} back into working order. +{} Upkeep experience.",
            task.xp
        )
    });
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

/// The only part of the restoration that happens without the Scribe. The clock
/// runs without marking the garden changed, because a save written every frame
/// of a two-minute season would be a save written for nothing; ripening is what
/// is worth recording, and worth interrupting the player to say.
fn grow_garden(time: Res<Time>, mut garden: ResMut<Garden>, mut journal: ResMut<Journal>) {
    if !garden.is_growing() {
        return;
    }
    let ripened = garden
        .bypass_change_detection()
        .tick(time.delta_secs())
        .len();
    if ripened == 0 {
        return;
    }
    garden.set_changed();
    journal.notice = Some(if ripened == 1 {
        "Grain is standing in one of the beds. Something in this valley is still willing."
            .to_owned()
    } else {
        format!("{ripened} beds have come ripe at once. You did not expect that.")
    });
}

/// A bed's art, and the rain butt's, are functions of saved state, so both are
/// reconciled here rather than written at every place that state changes.
fn sync_garden_plots(
    asset_server: Res<AssetServer>,
    garden: Res<Garden>,
    interior_state: Res<InteriorState>,
    mut plots: Query<(&mut GardenPlot, &mut Sprite)>,
    mut cisterns: Query<(&mut RainCistern, &mut Sprite), Without<GardenPlot>>,
) {
    for (mut plot, mut sprite) in &mut plots {
        let stage = garden.stage(&plot.id);
        let wanted = plot.art_for(stage, garden.nearly_ripe(&plot.id)).to_owned();
        if plot.art == wanted {
            continue;
        }
        sprite.image = asset_server.load(&wanted);
        plot.art = wanted;
    }
    let wanted = cistern_art(cistern_holds_water(&interior_state));
    for (mut cistern, mut sprite) in &mut cisterns {
        if cistern.art == wanted {
            continue;
        }
        cistern.art = wanted;
        sprite.image = asset_server.load(wanted);
    }
}

/// What a bed is asking for right now. A plot changes what it wants five times
/// over, so like the hearth it writes its own line rather than caching one.
fn garden_plot_prompt(plot: &GardenPlot, garden: &Garden, progression: &Progression) -> String {
    let stage = garden.stage(&plot.id);
    let Some(task) = plot.work_task(stage) else {
        return format!(
            "Nothing to do here — {}",
            if garden.nearly_ripe(&plot.id) {
                "the heads are filling out"
            } else {
                "the grain is barely up"
            }
        );
    };
    let missing = progression.shortfalls(&task);
    if missing.is_empty() {
        format!("R — {}     [{}]", stage.work(), task.requirements_text())
    } else {
        format!(
            "R — {}     [still needs {}]",
            stage.work(),
            missing.join(" · ")
        )
    }
}

/// The line the Scribe reads after one piece of garden work. Restoration prose
/// elsewhere is plain because the outcome is certain; here it is not, so each
/// step is written as something that could have failed and did not.
fn garden_work_notice(worked: garden::Worked, progression: &Progression) -> String {
    match worked.from {
        PlotStage::Paved => format!(
            "The slab comes up in plates and you lever them aside. Underneath, soil — dark, and deep, and from before. Whatever was here grew in it once. Stone: {}.",
            progression.supply(SupplyId::Stone)
        ),
        PlotStage::Fallow => "You draw the hoe through until the ground lies in rows. It gives more easily than it has any right to.".to_owned(),
        PlotStage::Tilled => format!(
            "Seed along the rows, covered over by hand. Seed left: {}.",
            progression.supply(SupplyId::Seed)
        ),
        PlotStage::Sown => format!(
            "The water goes in without a sound and does not run off. Canfuls left: {}.",
            progression.supply(SupplyId::Water)
        ),
        // Growing has no work; only the clock finishes it.
        PlotStage::Growing => "The grain is still coming up. Nothing here will be hurried.".to_owned(),
        PlotStage::Ripe => {
            let mut notice = format!(
                "You thresh it out by hand. Rations: {}. Two seeds back for the one that went in.",
                progression.supply(SupplyId::Ration)
            );
            if worked.bonus_rations > 0 {
                notice.push_str("\nThe bed gave more than it was asked for. You count it twice to be sure.");
            }
            notice
        }
    }
}

/// What tending a lit hearth would actually do, in the order somebody standing
/// in front of one would do it: the fire first, because a fire that goes out
/// takes the whole waystation with it, then the pot, then stretching the pot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HearthChore {
    FeedTheFire(upkeep::Fuel),
    AddARation,
    StretchThePot,
    Nothing,
}

/// One fire's worth of wood, phrased the way every other requirement line is.
const WOOD_FOR_A_NIGHT: &str = "1 fallen log or 3 kindling";

fn hearth_chore(state: &upkeep::Upkeep, progression: &Progression) -> HearthChore {
    state.wood_on_hand(progression).map_or_else(
        || {
            if state.can_add_a_ration(progression) {
                HearthChore::AddARation
            } else if state.can_stretch(progression) {
                HearthChore::StretchThePot
            } else {
                HearthChore::Nothing
            }
        },
        HearthChore::FeedTheFire,
    )
}

/// The hearth carries no `TaskSpec`, so it writes its own requirement line in the
/// same shape the worked stations use.
fn hearth_prompt(
    interior_state: &InteriorState,
    progression: &Progression,
    state: &upkeep::Upkeep,
) -> String {
    if hearth_is_lit(interior_state) {
        return match hearth_chore(state, progression) {
            HearthChore::FeedTheFire(upkeep::Fuel::Log) => {
                "E — put a log on the fire     [1 fallen log]".to_owned()
            }
            HearthChore::FeedTheFire(upkeep::Fuel::Kindling) => {
                "E — feed the fire     [3 kindling]".to_owned()
            }
            HearthChore::AddARation => "E — put a ration in the pot     [1 ration]".to_owned(),
            HearthChore::StretchThePot => {
                "E — let the pot down with water     [1 canful of water]".to_owned()
            }
            // A hearth with nothing left to give it says so the same way a job
            // with a missing plank does, and only while the fire is short.
            HearthChore::Nothing if state.banked_nights() == 0 => {
                format!("E — tend the hearth     [still needs {WOOD_FOR_A_NIGHT}]")
            }
            HearthChore::Nothing => "E — tend the hearth".to_owned(),
        };
    }
    let missing = hearth_blockers(interior_state, progression);
    if missing.is_empty() {
        format!("E — light the hearth     [{HEARTH_KINDLING} kindling · cleared chimney]")
    } else {
        format!(
            "E — tend the hearth     [still needs {}]",
            missing.join(" · ")
        )
    }
}

const fn cistern_art(holds_water: bool) -> &'static str {
    if holds_water {
        "world/rain_cistern.png"
    } else {
        "world/rain_cistern_damaged.png"
    }
}

fn cistern_holds_water(interior_state: &InteriorState) -> bool {
    interior_state
        .0
        .get(RAIN_CISTERN_STATE_KEY)
        .is_some_and(|state| state == "repaired")
}

/// Coopering, near enough: new staves off a plank and the hoops nailed back.
fn cistern_repair_task() -> progression::TaskSpec {
    progression::TaskSpec::for_kind("door")
}

/// The cistern owns its prompt for the same reason a bed does — what it wants
/// depends on whether it holds water yet.
fn cistern_prompt(interior_state: &InteriorState, progression: &Progression) -> String {
    let (work, task) = if cistern_holds_water(interior_state) {
        (
            "fill the can from the cistern",
            progression::TaskSpec::for_drawing_water(),
        )
    } else {
        ("rebuild the staved-in rain butt", cistern_repair_task())
    };
    let missing = progression.shortfalls(&task);
    if missing.is_empty() {
        format!("R — {work}     [{}]", task.requirements_text())
    } else {
        format!("R — {work}     [still needs {}]", missing.join(" · "))
    }
}

fn hearth_is_lit(interior_state: &InteriorState) -> bool {
    interior_state
        .0
        .get(OFFICE_HEARTH_STATE_KEY)
        .is_some_and(|state| state == "repaired")
}

/// Everything the hearth is still waiting on, phrased for a player. Both the
/// nearby prompt and the failed interaction read from this one list so the fire
/// can never refuse silently.
fn hearth_blockers(interior_state: &InteriorState, progression: &Progression) -> Vec<String> {
    let mut missing = Vec::new();
    if interior_state
        .0
        .get(OFFICE_CHIMNEY_STATE_KEY)
        .is_none_or(|state| state != "repaired")
    {
        missing.push("a cleared chimney".to_owned());
    }
    let kindling = progression.supply(SupplyId::Kindling);
    if kindling < HEARTH_KINDLING {
        missing.push(format!("{HEARTH_KINDLING} kindling (you have {kindling})"));
    }
    missing
}

/// One press at a lit hearth. The chore is whatever the fire and the pot want
/// most, and the line reports what was actually done rather than what could have
/// been — including the case where the answer is nothing at all.
fn tend_the_hearth(
    state: &mut upkeep::Upkeep,
    progression: &mut Progression,
    sheltered: bool,
) -> String {
    match hearth_chore(state, progression) {
        HearthChore::FeedTheFire(_) => {
            state.feed_the_fire(progression, sheltered);
            match state.banked_nights() {
                0 | 1 => "You build it back up. It will hold tonight.".to_owned(),
                nights => format!(
                    "You build it back up. Wood in for {nights} nights, if nothing goes wrong."
                ),
            }
        }
        HearthChore::AddARation => {
            state.add_a_ration(progression);
            format!(
                "In it goes, and the pot takes it the way it takes everything. {}.",
                capitalised(&state.pot().describe_at_length())
            )
        }
        HearthChore::StretchThePot => {
            state.stretch_the_pot(progression);
            format!(
                "A canful, and an hour over the heat to make it worth eating. {}.",
                capitalised(&state.pot().describe_at_length())
            )
        }
        HearthChore::Nothing if state.banked_nights() == 0 => {
            "It is burning down and there is nothing here to put on it.".to_owned()
        }
        HearthChore::Nothing => {
            format!("The fire holds. {}.", capitalised(&state.summary(true)))
        }
    }
}

/// Whether the office is a room again rather than four broken walls. Nothing
/// announces this; the only sign of it is that the woodpile lasts longer.
fn walls_hold_the_heat(interior: &interior::InteriorMap, interior_state: &InteriorState) -> bool {
    let scene = interior.id();
    let mut walls = interior
        .mutable_elements()
        .iter()
        .filter(|element| element.id.starts_with(OFFICE_WALL_PREFIX))
        .peekable();
    walls.peek().is_some()
        && walls.all(|element| {
            interior_state
                .0
                .get(&format!("{scene}/{}", element.id))
                .is_some_and(|state| state == "repaired")
        })
}

/// A sentence that starts with a lower-case clause needs a capital on it.
fn capitalised(text: &str) -> String {
    let mut characters = text.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + characters.as_str()
    })
}

/// The Scribe's own words about why the fire will not take. Built from the same
/// blocker list the prompt uses, so the complaint and the requirement line can
/// never disagree with each other.
fn hearth_complaint(missing: &[String]) -> String {
    let flue = missing.iter().any(|item| item.contains("chimney"));
    let fuel = missing.iter().any(|item| item.contains("kindling"));
    match (flue, fuel) {
        (true, true) => {
            "I can't light this. Nothing dry enough to catch, and no telling what's clogging up that chimney.".to_owned()
        }
        (true, false) => {
            "I can't light this fire. No telling what's clogging up that chimney — it would only fill the room with smoke.".to_owned()
        }
        (false, true) => {
            "The flue draws clean. Nothing here dry enough to catch, though.".to_owned()
        }
        (false, false) => "Something is stopping it, and I cannot see what.".to_owned(),
    }
}

const fn interaction_key_matches(
    kind: InteractableKind,
    interact_pressed: bool,
    repair_pressed: bool,
) -> bool {
    match kind {
        InteractableKind::InteriorRepairable
        | InteractableKind::ExteriorRepairable
        | InteractableKind::Tree
        | InteractableKind::Sawbuck
        | InteractableKind::StoneOutcrop
        | InteractableKind::GardenPlot
        | InteractableKind::RainCistern => repair_pressed,
        // A desk can be searched or mended and a tool taken or repaired, so both
        // keys reach them and the interaction works out which was meant.
        InteractableKind::Tool | InteractableKind::Desk => interact_pressed || repair_pressed,
        _ => interact_pressed,
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn handle_interaction(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    nearby: Res<Nearby>,
    mut journal: ResMut<Journal>,
    mut popup: ResMut<NarrativePopup>,
    portfolio: Res<Portfolio>,
    interior: Res<interior::InteriorMap>,
    motel: Res<interior::MotelExteriorMap>,
    tool_shed: Res<interior::ToolShedExteriorMap>,
    asset_server: Res<AssetServer>,
    mut resources: InteractionResources,
    mut exterior_obstacles: ResMut<ExteriorObstacles>,
    mut queries: InteractionQueries,
) {
    if popup.is_open() || portfolio.is_open() {
        return;
    }
    let interact_pressed = keys.just_pressed(KeyCode::KeyE);
    let repair_pressed = keys.just_pressed(KeyCode::KeyR);
    // Only when nothing underfoot wants the key does R mean "work on what I am
    // carrying". Standing over a job, the job wins.
    let claimed_by_the_world = nearby.0.is_some_and(|entity| {
        queries.interactables.get(entity).is_ok_and(|target| {
            interaction_key_matches(target.kind, interact_pressed, repair_pressed)
        })
    });
    if repair_pressed && !claimed_by_the_world {
        if let Some((id, tool)) = resources.progression.carried_broken_tool() {
            mend_tool(
                &mut commands,
                &asset_server,
                &mut resources.progression,
                &mut journal,
                &mut queries.player_animation,
                &id,
                tool.label(),
            );
        }
        return;
    }
    let Some(entity) = nearby.0 else {
        return;
    };
    let Ok(mut target) = queries.interactables.get_mut(entity) else {
        return;
    };
    if !interaction_key_matches(target.kind, interact_pressed, repair_pressed) {
        return;
    }
    if let Ok((portable, label)) = queries.portable_tools.get(entity) {
        let Some(record) = resources.progression.tool_record(&portable.id).cloned() else {
            return;
        };
        if repair_pressed {
            mend_tool(
                &mut commands,
                &asset_server,
                &mut resources.progression,
                &mut journal,
                &mut queries.player_animation,
                &portable.id,
                &label.0,
            );
            return;
        }
        match resources.progression.pick_up_tool(&portable.id) {
            Ok(tool) => {
                target.consumed = true;
                commands.entity(entity).insert(Visibility::Hidden);
                journal.notice = Some(if record.condition == ToolCondition::Broken {
                    format!(
                        "You take the {}. I'll need to repair this {} before I can use it. Carried tools: {}/{}.",
                        label.0,
                        record.tool.label(),
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
            Err(reason) => journal.notice = Some(reason),
        }
        return;
    }
    if let Ok(tree) = queries.choppable_trees.get(entity) {
        let task = progression::TaskSpec::for_tree_chopping();
        let outcome = match resources.progression.attempt(&task) {
            Ok(outcome) => outcome,
            Err(reason) => {
                journal.notice = Some(format!(
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
        journal.notice = Some(format!(
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
            journal.notice = Some(format!(
                "You cannot mill a plank yet. {reason}\nRequires: {}.",
                task.requirements_text()
            ));
            return;
        }
        if let Ok(mut animation) = queries.player_animation.single_mut() {
            start_tool_animation(&mut animation, ToolWorkAnimation::Axe);
        }
        journal.notice = Some(format!(
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
            journal.notice = Some(format!(
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
        journal.notice = Some(format!(
            "The pick rings and the seam gives. You carry off {}. Stone: {}.",
            task.yields_text(),
            resources.progression.supply(SupplyId::Stone)
        ));
        return;
    }
    if let Ok(plot) = queries.garden_plots.get(entity) {
        let plot_id = plot.id.clone();
        let stage = resources.garden.stage(&plot_id);
        let Some(task) = plot.work_task(stage) else {
            journal.notice = Some(garden_work_notice(
                garden::Worked {
                    from: stage,
                    to: stage,
                    bonus_rations: 0,
                },
                &resources.progression,
            ));
            return;
        };
        if let Err(reason) = resources.progression.attempt(&task) {
            journal.notice = Some(format!(
                "You cannot {} yet. {reason}\nRequires: {}.",
                stage.work(),
                task.requirements_text()
            ));
            return;
        }
        let worked = resources.garden.advance(&plot_id);
        if worked.bonus_rations > 0 {
            resources
                .progression
                .add_supply(SupplyId::Ration, worked.bonus_rations);
        }
        start_task_animation(
            &task,
            &mut commands,
            &asset_server,
            &mut queries.player_animation,
        );
        journal.notice = Some(garden_work_notice(worked, &resources.progression));
        return;
    }
    if target.kind == InteractableKind::RainCistern {
        if !cistern_holds_water(&resources.interior_state) {
            let task = cistern_repair_task();
            if let Err(reason) = resources.progression.attempt(&task) {
                journal.notice = Some(format!(
                    "The rain butt is staved in and holds nothing. {reason}\nRequires: {}.",
                    task.requirements_text()
                ));
                return;
            }
            resources
                .interior_state
                .0
                .insert(RAIN_CISTERN_STATE_KEY.to_owned(), "repaired".to_owned());
            start_task_animation(
                &task,
                &mut commands,
                &asset_server,
                &mut queries.player_animation,
            );
            journal.notice = Some(
                "New staves, the hoops driven back down. It will take whatever the storms give, which is the one thing they give freely."
                    .to_owned(),
            );
            return;
        }
        let task = progression::TaskSpec::for_drawing_water();
        if let Err(reason) = resources.progression.attempt(&task) {
            journal.notice = Some(format!(
                "You cannot draw from the cistern yet. {reason}\nRequires: {}.",
                task.requirements_text()
            ));
            return;
        }
        start_task_animation(
            &task,
            &mut commands,
            &asset_server,
            &mut queries.player_animation,
        );
        journal.notice = Some(format!(
            "Rain the valley had no other use for. You fill the can to the lip. Canfuls of water: {}.",
            resources.progression.supply(SupplyId::Water)
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
                journal.notice = Some(format!(
                    "Dry wood, sheltered beneath the old growth. Kindling: {}.",
                    resources.progression.supply(SupplyId::Kindling)
                ));
            }
            InteractableKind::Log => {
                journal.notice = Some(format!(
                    "A fallen log, weathered but sound. Logs: {}.",
                    resources.progression.supply(SupplyId::Log)
                ));
            }
            InteractableKind::Plank => {
                journal.notice = Some(format!(
                    "Old cedar, still sound. Planks: {}.",
                    resources.progression.supply(SupplyId::Plank)
                ));
            }
            InteractableKind::Tool => {
                journal.notice = Some(
                    "An old ladder, silvered by weather but still sturdy. It can reach the motel roof."
                        .to_owned(),
                );
            }
            InteractableKind::Forage => {
                journal.notice = Some(format!(
                    "Not much. Enough for a day, eaten carefully. More than the ash has offered since you came down off the mountain. Rations: {}.",
                    resources.progression.supply(SupplyId::Ration)
                ));
            }
            _ => {}
        }
        return;
    }
    match target.kind {
        InteractableKind::Sign => {
            journal.say(
                "Way Station Motel. A shelter-name from the old speech. Under it, unlit and \
                 unretracted for a hundred years: VACANCY.",
            );
        }
        InteractableKind::SeedStore => {
            let key = format!("tool-shed-interior/{SHED_SEED_STORE_ID}");
            if resources.interior_state.0.contains_key(&key) {
                journal.notice = Some(
                    "The shelf is bare now. Whatever seed the waystation sees after this has to be grown, traded for, or given."
                        .to_owned(),
                );
                return;
            }
            resources
                .interior_state
                .0
                .insert(key, DISCOVERY_FOUND_STATE.to_owned());
            resources
                .progression
                .add_supply(SupplyId::Seed, SHED_SEED_STORE);
            commands.entity(entity).insert(Visibility::Hidden);
            journal.notice = Some(format!(
                "A sack of seed grain, kept dry on the high shelf by somebody who did not get to sow it. Seed: {}.",
                resources.progression.supply(SupplyId::Seed)
            ));
        }
        InteractableKind::Hearth => {
            if hearth_is_lit(&resources.interior_state) {
                journal.notice = Some(tend_the_hearth(
                    &mut resources.upkeep,
                    &mut resources.progression,
                    walls_hold_the_heat(&interior, &resources.interior_state),
                ));
                return;
            }
            let missing = hearth_blockers(&resources.interior_state, &resources.progression);
            if !missing.is_empty() {
                // The Scribe says what is wrong with the fire in front of them.
                // Where to fix it is the player's problem, and finding out is the
                // part worth having.
                journal.say(format!(
                    "You crouch at the hearth. {}",
                    hearth_complaint(&missing)
                ));
                return;
            }
            if !resources
                .progression
                .spend_supply(SupplyId::Kindling, HEARTH_KINDLING)
            {
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
            // A lit hearth is not a finished job. It goes on wanting wood every
            // night it burns, so it stays the nearest thing worth walking up to.
            // The kindling that lit it is also the first night's fuel.
            resources.upkeep.lay_a_fire();
            journal.say(
                "Flame takes. Warm light reaches into a room untouched for centuries. Whatever else this smoke does, it will be visible from the ridge.",
            );
        }
        InteractableKind::Desk => {
            let desk_is_broken = queries
                .mutable_elements
                .get(entity)
                .is_ok_and(|(instance, ..)| instance.state != "repaired");
            if repair_pressed && desk_is_broken {
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
                            journal.notice = Some(format!(
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
                journal.say(
                    "The desk stands square again; the first careful carpentry lesson is learned.",
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
            journal.notice = Some(if discoveries.is_empty() {
                "The old desk has already yielded its secrets.".to_owned()
            } else {
                discoveries.join("\n\n")
            });
        }
        InteractableKind::BibleNightstand => {
            read_the_book(&mut resources, &mut popup, &mut journal);
        }
        InteractableKind::Visitor => {
            greet_visitor(&mut resources.visitors, &mut journal);
        }
        InteractableKind::Bed => {
            sleep_here(&mut resources, &mut journal);
        }
        InteractableKind::Salvage => {
            let label = queries
                .interaction_labels
                .get(entity)
                .map_or_else(|_| "hiding place".to_owned(), |label| label.0.clone());
            if let Ok(scene_key) = queries.interaction_ids.get(entity) {
                resources
                    .interior_state
                    .0
                    .insert(scene_key.0.clone(), DISCOVERY_FOUND_STATE.to_owned());
            }
            search_for_salvage(&mut resources, &mut popup, &mut journal, &label);
            target.consumed = true;
            commands.entity(entity).insert(Visibility::Hidden);
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
                    journal.notice = Some(format!(
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
                journal.notice = Some(format!("{} cannot be repaired yet.", element.label));
                return;
            }
            start_task_animation(
                &element.task,
                &mut commands,
                &asset_server,
                &mut queries.player_animation,
            );
            target.consumed = true;
            journal.notice = Some(task_success_notice(element, &outcome));
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
                    journal.notice = Some(format!(
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
                journal.notice = Some(format!("{} cannot be repaired yet.", element.label));
                return;
            }
            start_task_animation(
                &element.task,
                &mut commands,
                &asset_server,
                &mut queries.player_animation,
            );
            target.consumed = true;
            journal.notice = Some(task_success_notice(element, &outcome));
        }
        _ => {
            journal.notice = Some("There may be a use for this later.".to_owned());
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
    set_scene_element_state(
        asset_server,
        Some(interior_state),
        element,
        "repaired",
        center,
        instance,
        sprite,
        transform,
        visibility,
    )
}

/// Puts a spawned element into one of its authored states, art and all. Repairs
/// go one way and record themselves; a fire going out goes the other way and is
/// recorded by whoever put it out, so the state map is optional here.
#[allow(clippy::too_many_arguments)]
fn set_scene_element_state(
    asset_server: &AssetServer,
    interior_state: Option<&mut InteriorState>,
    element: &interior::MutableElement,
    state: &str,
    center: Vec2,
    instance: &mut MutableSceneElement,
    sprite: &mut Sprite,
    transform: &mut Transform,
    visibility: &mut Visibility,
) -> bool {
    let Some(visual) = element.states.get(state) else {
        return false;
    };
    if let Some(path) = &visual.image_path {
        sprite.image = asset_server.load(path.clone());
    }
    sprite.custom_size = Some(visual.size.max(Vec2::ONE));
    transform.translation.x = center.x;
    transform.translation.y = center.y;
    *visibility = if visual.visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    state.clone_into(&mut instance.state);
    if let Some(interior_state) = interior_state {
        interior_state.0.insert(
            format!("{}/{}", instance.scene_id, instance.id),
            instance.state.clone(),
        );
    }
    true
}

/// Which rooms have a roof and a door that latches, even before a single repair.
/// Offering one is the largest thing the Scribe owns to give.
const GUEST_ROOMS: [(interior::InteriorId, &str); 2] = [
    (interior::InteriorId::Room01, "room one"),
    (interior::InteriorId::Room06, "room six"),
];

/// A guest can only be shown to a room the Scribe can open, which means the
/// brass keys, which means having searched the office desk.
fn offerable_room(motel_access: &MotelAccess) -> Option<(interior::InteriorId, &'static str)> {
    motel_access.keys_found.then(|| GUEST_ROOMS[0])
}

/// Digits `1`–`9`, as a zero-based choice.
fn digit_pressed(keys: &ButtonInput<KeyCode>) -> Option<usize> {
    const DIGITS: [KeyCode; 9] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    DIGITS.iter().position(|&digit| keys.just_pressed(digit))
}

#[derive(SystemParam)]
struct VisitInput<'w> {
    visitors: ResMut<'w, Visitors>,
    journal: ResMut<'w, Journal>,
    collection: ResMut<'w, Collection>,
    progression: ResMut<'w, Progression>,
    upkeep: ResMut<'w, upkeep::Upkeep>,
    motel_access: Res<'w, MotelAccess>,
    inbox: Res<'w, InterpretInbox>,
}

fn handle_visit_input(
    keys: Res<ButtonInput<KeyCode>>,
    portfolio: Res<Portfolio>,
    mut popup: ResMut<NarrativePopup>,
    mut visit: VisitInput,
) {
    if portfolio.is_open() {
        return;
    }
    if popup.is_open() {
        popup.handle_input(
            keys.just_pressed(KeyCode::KeyE)
                || keys.just_pressed(KeyCode::Space)
                || keys.just_pressed(KeyCode::Escape),
        );
        return;
    }
    let Some(stage) = visit.visitors.party.as_ref().map(|party| party.stage) else {
        return;
    };
    match stage {
        VisitStage::Telling => advance_telling(&keys, &mut visit),
        VisitStage::Deciding => decide_hospitality(&keys, &mut visit),
        VisitStage::Choosing => choose_a_card(&keys, &mut visit),
        _ => {}
    }
}

/// What the Scribe can put in front of a stranger, which is whatever is nearest
/// to hand and thinnest. One list writes both the offer the overlay shows and
/// what the key does, so the two can never promise different things.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FoodOffer {
    /// A bowl out of the pot, with another one left for the Scribe.
    Bowl,
    /// The only bowl there is, which can only be shared by halving it.
    LastBowl,
    Ration,
    LastRation,
    Nothing,
}

fn food_offer(state: &upkeep::Upkeep, progression: &Progression) -> FoodOffer {
    if state.can_ladle_a_bowl() {
        FoodOffer::Bowl
    } else if state.only_the_last_bowl() {
        FoodOffer::LastBowl
    } else {
        match progression.supply(SupplyId::Ration) {
            0 => FoodOffer::Nothing,
            1 => FoodOffer::LastRation,
            _ => FoodOffer::Ration,
        }
    }
}

impl FoodOffer {
    /// The line on the offer list. It names the cost, because the cost is the
    /// decision — there is nothing else to weigh.
    fn line(self, state: &upkeep::Upkeep, progression: &Progression) -> String {
        match self {
            Self::Bowl => format!("1  Ladle out a bowl ({} in the pot)", state.pot().bowls()),
            Self::LastBowl => "1  Share the last of the pot, half each".to_owned(),
            Self::Ration => format!(
                "1  Share food ({} in the pack)",
                progression.supply(SupplyId::Ration)
            ),
            Self::LastRation => "1  Split your last ration, half each".to_owned(),
            Self::Nothing => "1  — you have nothing to eat yourself".to_owned(),
        }
    }
}

fn advance_telling(keys: &ButtonInput<KeyCode>, visit: &mut VisitInput) {
    if !keys.just_pressed(KeyCode::Space) {
        return;
    }
    let Some(party) = visit.visitors.party.as_mut() else {
        return;
    };
    let spoken = party.spoken().len();
    party.line += 1;
    if party.line < spoken {
        return;
    }
    party.has_spoken = true;
    party.stage = VisitStage::Listening;
    begin_interpretation(&party.vignette, &visit.inbox);
}

fn decide_hospitality(keys: &ButtonInput<KeyCode>, visit: &mut VisitInput) {
    if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Escape) {
        let farewell = visit
            .visitors
            .party
            .as_ref()
            .map(farewell_notice)
            .unwrap_or_default();
        visit.visitors.finish_deciding();
        visit.journal.say(farewell);
        return;
    }
    let Some(choice) = digit_pressed(keys) else {
        return;
    };
    let offer = food_offer(&visit.upkeep, &visit.progression);
    let room = offerable_room(&visit.motel_access);
    let Some(party) = visit.visitors.party.as_mut() else {
        return;
    };
    match choice {
        // Sharing food is deliberately allowed down to the last of it. The
        // Scribe going hungry for a stranger is the whole point of the passage
        // that put the idea in their head — and when there is only one bowl,
        // halving it is the offer, because that is what a person does.
        0 if offer != FoodOffer::Nothing && !party.given.food => {
            let who = party.address();
            party.given.food = true;
            let line = match offer {
                FoodOffer::Bowl => {
                    let bowl = visit.upkeep.pot().quality();
                    visit.upkeep.ladle_a_bowl();
                    format!(
                        "You ladle out a bowl — {bowl}. {who} eats slowly, the way people do when they have learned not to trust a full stomach."
                    )
                }
                FoodOffer::LastBowl => {
                    visit.upkeep.share_the_last();
                    format!(
                        "You divide the last of the pot in front of {who}, down the middle, so there is nothing to argue about. Neither of you has eaten. Both of you have."
                    )
                }
                FoodOffer::Ration => {
                    visit.progression.spend_supply(SupplyId::Ration, 1);
                    format!(
                        "You divide what you have. {who} eats slowly, the way people do when they have learned not to trust a full stomach."
                    )
                }
                FoodOffer::LastRation => {
                    visit.upkeep.split_a_ration(&mut visit.progression);
                    format!(
                        "You break the last of it in two and hold out a half. {who} looks at both pieces long enough to be certain they are the same size."
                    )
                }
                FoodOffer::Nothing => String::new(),
            };
            visit.journal.say(line);
        }
        1 if party.given.room.is_none() => {
            if let Some((id, label)) = room {
                party.given.room = Some(label.to_owned());
                visit.journal.say(format!(
                    "You hold out the key to {label}. The roof is sound and the door still latches, which is more than the road offers. {} looks at it for a long moment before taking it.",
                    party.address()
                ));
                let _ = id;
            } else {
                visit.journal.say(
                    "You have nowhere to put anybody. The doors are locked and you do not have the keys.",
                );
            }
        }
        2 if party.given.card.is_none() => {
            party.stage = VisitStage::Choosing;
        }
        _ => {}
    }
}

fn choose_a_card(keys: &ButtonInput<KeyCode>, visit: &mut VisitInput) {
    if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Escape) {
        if let Some(party) = visit.visitors.party.as_mut() {
            party.stage = VisitStage::Deciding;
        }
        return;
    }
    let Some(choice) = digit_pressed(keys) else {
        return;
    };
    let Some(print) = visit.collection.on_hand().get(choice).copied() else {
        return;
    };
    visit.collection.give(&print.id);
    let Some(party) = visit.visitors.party.as_mut() else {
        return;
    };
    party.given.card = Some(print.id.clone());
    party.stage = VisitStage::Deciding;
    // Most people out there cannot read. The Scribe says the words aloud until
    // the card can be carried without them.
    visit.journal.say(format!(
        "You put the block-print into {}'s hands and read it aloud, twice, until they can say it back.\n\n\u{201c}{}\u{201d}\n{}",
        party.address(),
        print.verse,
        print.reference
    ));
}

/// Walking up to somebody who is standing in your yard. There is no prompt
/// telling the player to do this and no penalty for not doing it.
fn greet_visitor(visitors: &mut Visitors, journal: &mut Journal) {
    let Some(party) = visitors.party.as_mut() else {
        return;
    };
    if !party.can_be_greeted() {
        return;
    }
    journal.notice = None;
    if party.has_spoken {
        // A guest saying goodbye in the morning does not recite their life again.
        party.stage = VisitStage::Deciding;
    } else {
        party.stage = VisitStage::Telling;
        party.line = 0;
    }
}

/// Opening the book. The first time is a discovery and the Scribe says so; every
/// time after, it is only reading, which is what a book is for. It never leaves
/// room three.
fn read_the_book(
    resources: &mut InteractionResources,
    popup: &mut NarrativePopup,
    journal: &mut Journal,
) {
    let first_time = !bible_found(&resources.interior_state);
    record_bible_discovery(&mut resources.interior_state);
    if first_time {
        popup.present(NarrativeCard::Item(DiscoveredItem::GideonBible));
    }
    let Some(reading) = resources.readings.open(&mut resources.chance) else {
        return;
    };
    popup.present(NarrativeCard::Passage {
        title: reading.reference.clone(),
        body: format!(
            "\u{201c}{}\u{201d}\n\n{}",
            reading.verse, reading.reflection
        ),
    });
    journal.say("You put the book back exactly where it was.");
}

/// Searching somewhere nobody has searched since the world ended. Almost
/// everything found is worthless, and the reading of it is the point.
fn search_for_salvage(
    resources: &mut InteractionResources,
    popup: &mut NarrativePopup,
    journal: &mut Journal,
    label: &str,
) {
    let Some(find) = resources.salvaged.draw(&mut resources.chance) else {
        return;
    };
    popup.present(NarrativeCard::Salvage {
        title: find.label.clone(),
        body: find.line.clone(),
    });
    if let Some(reward) = find.reward {
        resources.progression.add_supply(reward.item, reward.amount);
        journal.say(format!(
            "The {label} gives up {} {}.",
            reward.amount,
            reward.item.label()
        ));
    } else {
        journal.say(format!("The {label} held on to that for a very long time."));
    }
}

/// Lying down. The night goes by at once, because the alternative is standing in
/// a dark room for ninety seconds waiting for a number to change.
fn sleep_here(resources: &mut InteractionResources, journal: &mut Journal) {
    if !resources.clock.is_bedtime() {
        journal
            .say("There is too much light left in the day to lie down, and too much left undone.");
        return;
    }
    // A bed under a sound roof is the whole of what makes a night dry. The
    // settlement at the turn of the day reads this and spends it.
    resources.upkeep.slept_here();
    let morning = resources.clock.sleep_until_morning();
    journal.say(format!(
        "You sleep, badly at first and then properly, under a roof you mended yourself. Day {morning}."
    ));
}

/// The last thing said about a visit, which depends only on what was actually
/// given. No score, no approval — just an honest account.
fn farewell_notice(party: &visitors::Party) -> String {
    let who = party.address();
    let mut given = Vec::new();
    if party.given.food {
        given.push("fed");
    }
    if party.given.room.is_some() {
        given.push("housed");
    }
    if party.given.card.is_some() {
        given.push("sent off with words");
    }
    match given.as_slice() {
        [] => format!(
            "{who} thanks you for the fire and goes back to the road. You did not have to give anything, and did not."
        ),
        _ => format!(
            "{who} was {} here. Whatever that is worth out there, it is more than this valley had yesterday.",
            given.join(", ")
        ),
    }
}

fn begin_interpretation(vignette_id: &str, inbox: &InterpretInbox) {
    if let Ok(mut value) = inbox.0.lock() {
        *value = None;
    }
    let inbox = Arc::clone(&inbox.0);
    let request = InterpretRequest {
        vignette_id: vignette_id.to_owned(),
        language: player_language(),
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

/// The language the traveler's passage should arrive in. The browser already
/// knows it and the desktop shell already says it, so nothing is asked of the
/// player. The server treats an unmapped or missing tag as English, which means
/// a wrong guess here costs a translation and never a passage.
#[cfg(target_arch = "wasm32")]
fn player_language() -> Option<String> {
    web_sys::window().and_then(|window| window.navigator().language())
}

/// `LANG` arrives as `en_US.UTF-8`; the server wants the tag in front of the
/// encoding. `C` and `POSIX` mean "unset" rather than a language.
#[cfg(not(target_arch = "wasm32"))]
fn player_language() -> Option<String> {
    let value = std::env::var("LANG").ok()?;
    let tag = value.split('.').next().unwrap_or_default();
    if tag.is_empty() || tag == "C" || tag == "POSIX" {
        return None;
    }
    Some(tag.to_owned())
}

/// Where the listening lives.
///
/// Served by our own server, the page and the API share an origin and a relative
/// path is both correct and the least to go wrong. The submitted demo link is a
/// static host that has no `/api/interpret` to answer, and it cannot be moved, so
/// that build is told at compile time to ask elsewhere. `WAYSTATION_API_ORIGIN`
/// unset or empty means same-origin, which is what local development wants.
///
/// A build that should have been given an origin and was not is the dangerous
/// case: every request 404s, every traveler falls back to the reviewed fixture,
/// and the game looks exactly like one that is working. CI checks the built
/// bundle for the origin rather than trusting that the variable was set.
#[cfg(target_arch = "wasm32")]
fn api_url(path: &str) -> String {
    match option_env!("WAYSTATION_API_ORIGIN") {
        Some(origin) if !origin.trim().is_empty() => {
            format!("{}{path}", origin.trim().trim_end_matches('/'))
        }
        _ => path.to_owned(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn api_url(path: &str) -> String {
    format!("http://127.0.0.1:7777{path}")
}

/// The listening. Gloo reads the need beneath the authored words and the answer
/// only ever marks which card the Scribe reaches for first; the player is free
/// to hand over a different one, or none.
fn poll_interpretation(mut visitors: ResMut<Visitors>, inbox: Res<InterpretInbox>) {
    let Some(party) = visitors.party.as_mut() else {
        return;
    };
    if party.stage != VisitStage::Listening {
        return;
    }
    let Some(result) = inbox.0.lock().ok().and_then(|mut slot| slot.take()) else {
        return;
    };
    let response = match result {
        Ok(response) => response,
        Err(error) => {
            bevy::log::warn!("API request failed: {error}; using reviewed fixture");
            fixture_response(&party.vignette).expect("every vignette has a fixture")
        }
    };
    party.need = Some(response);
    party.stage = VisitStage::Deciding;
}

/// Runs the clock, and does the night's business whenever the date changes —
/// whether the player slept through it or stood outside watching it happen.
#[allow(clippy::too_many_arguments)]
fn advance_clock(
    time: Res<Time>,
    readings: Res<Readings>,
    mut interior_state: ResMut<InteriorState>,
    mut clock: ResMut<Clock>,
    mut last_day: Local<u32>,
    mut visitors: ResMut<Visitors>,
    mut collection: ResMut<Collection>,
    mut chance: ResMut<Chance>,
    mut journal: ResMut<Journal>,
    mut progression: ResMut<Progression>,
    mut upkeep: ResMut<upkeep::Upkeep>,
    mut dirty: ResMut<SceneVisualsDirty>,
    mut pickups: Query<(&WorldPickup, &mut Interactable, &mut Visibility)>,
) {
    clock.tick(time.delta_secs());
    if *last_day == clock.day {
        return;
    }
    // Zero is the first frame of a session, not a night that passed.
    let starting_up = *last_day == 0;
    *last_day = clock.day;
    if starting_up {
        return;
    }
    let fire_was_lit = hearth_is_lit(&interior_state);
    if fire_was_lit {
        visitors.nights_of_smoke += 1;
    }
    visitors.wake_guests();
    // What the night cost. This is the only thing in the game that spends
    // without being asked, which is the point of it.
    let night = upkeep.settle_night(&mut progression, fire_was_lit);
    if night.fire_went_out {
        // The state map is the truth; the sprite catches up in
        // `reconcile_scene_visuals`, whether or not anybody is in the room.
        interior_state
            .0
            .insert(OFFICE_HEARTH_STATE_KEY.to_owned(), "damaged".to_owned());
        dirty.0 = true;
    }
    regrow_the_valley(&mut progression, &mut chance, &mut pickups);
    journal.say(night.line(clock.day));
    // The block-cutting is the Scribe's own business: nobody asked for it, and
    // it only happens once there is something read to cut. A night spent cold or
    // hungry is not one anybody has hands for it in.
    if night.was_good() && readings.has_read_anything() {
        if let Some(print) = collection.cut_a_block(&mut chance, readings.dwelling_on.as_deref()) {
            journal.say(format!(
                "You cut a block last night, badly at first. \u{201c}{}\u{201d} — {}. It kept your hands busy, which was the point.",
                print.title, print.reference
            ));
        }
    }
}

/// Which of the valley's pickups come back. Deadfall keeps falling, dry wood
/// keeps blowing in under the old growth, and the plants that survived the ash
/// go on doing whatever it is they do. A standing tree that has been felled is
/// felled, and quarried stone does not grow back.
const fn regrows(kind: InteractableKind) -> bool {
    matches!(
        kind,
        InteractableKind::Forage | InteractableKind::Kindling | InteractableKind::Log
    )
}

/// One thing a night, chosen at random from what has been taken. It is a slow
/// enough hand that a valley stripped bare stays stripped for a while, and
/// generous enough that it can never be stripped for good.
fn regrow_the_valley(
    progression: &mut Progression,
    chance: &mut Chance,
    pickups: &mut Query<(&WorldPickup, &mut Interactable, &mut Visibility)>,
) {
    let taken = pickups
        .iter()
        .filter(|(pickup, interactable, _)| {
            regrows(interactable.kind) && progression.pickup_collected(&pickup.id)
        })
        .count();
    if taken == 0 {
        return;
    }
    let wanted = chance.below(taken);
    let mut seen = 0;
    for (pickup, mut interactable, mut visibility) in pickups.iter_mut() {
        if !regrows(interactable.kind) || !progression.pickup_collected(&pickup.id) {
            continue;
        }
        if seen == wanted {
            progression.forget_pickup(&pickup.id);
            interactable.consumed = false;
            *visibility = Visibility::Visible;
            return;
        }
        seen += 1;
    }
}

/// Puts a spawned element back in step with the state map when something other
/// than an interaction moved it — which, so far, is the morning after a fire
/// that nobody fed.
fn reconcile_scene_visuals(
    asset_server: Res<AssetServer>,
    interior: Res<interior::InteriorMap>,
    interior_state: Res<InteriorState>,
    mut dirty: ResMut<SceneVisualsDirty>,
    mut elements: Query<
        (
            &mut MutableSceneElement,
            &mut Sprite,
            &mut Transform,
            &mut Visibility,
        ),
        Without<Player>,
    >,
) {
    if !dirty.0 {
        return;
    }
    dirty.0 = false;
    for (mut instance, mut sprite, mut transform, mut visibility) in &mut elements {
        let key = format!("{}/{}", instance.scene_id, instance.id);
        let Some(wanted) = interior_state.0.get(&key) else {
            continue;
        };
        if wanted == &instance.state {
            continue;
        }
        let Some(element) = interior.mutable_element(&instance.id) else {
            continue;
        };
        let Some(visual) = element.states.get(wanted.as_str()) else {
            continue;
        };
        let center = interior.element_center(element, visual.size);
        let wanted = wanted.clone();
        set_scene_element_state(
            &asset_server,
            None,
            element,
            &wanted,
            center,
            &mut instance,
            &mut sprite,
            &mut transform,
            &mut visibility,
        );
    }
}

/// Lays the failing light over everything. Kept below the parchment panels so a
/// night scene is still a readable screen.
fn sync_daylight(clock: Res<Clock>, mut tint: Query<&mut BackgroundColor, With<NightTint>>) {
    if let Ok(mut colour) = tint.single_mut() {
        colour.0 = clock.tint();
    }
}

/// What pressing E at a stranger will actually do, which depends on whether
/// they have got here yet and whether they have already spoken.
fn visitor_prompt(visitors: &Visitors) -> &'static str {
    match visitors
        .party
        .as_ref()
        .map(|party| (party.stage, party.has_spoken))
    {
        Some((VisitStage::Approaching, _)) => "They are still coming down the road.",
        Some((VisitStage::Waiting, true)) => "E — see them off",
        _ => "E — go and speak to them",
    }
}

/// True while a conversation owns the screen.
fn visitor_holds_the_screen(visitors: &Visitors) -> bool {
    visitors
        .party
        .as_ref()
        .is_some_and(|party| party.stage.holds_the_screen())
}

/// Rolls for arrivals, walks parties in and out, and counts down the patience of
/// anyone standing in the open.
#[allow(clippy::too_many_arguments)]
fn run_visits(
    time: Res<Time>,
    clock: Res<Clock>,
    interior_state: Res<InteriorState>,
    mut chance: ResMut<Chance>,
    mut visitors: ResMut<Visitors>,
    mut journal: ResMut<Journal>,
    rehearsal: Res<rehearsal::Rehearsal>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut bodies: Query<(
        &VisitorBody,
        &mut Transform,
        &mut Sprite,
        &mut Visibility,
        Has<npc_art::ComposingBody>,
    )>,
) {
    let fire_is_lit = hearth_is_lit(&interior_state);
    visitors.roll_for_today(*clock, fire_is_lit, &mut chance);
    if visitors.arrival_is_due(*clock) {
        let party = visitors.arrive_wanted(&mut chance, visitors::CURRENT_ERA, &rehearsal.wanted());
        // What the Scribe can make out from across the court: the shape of the
        // arrival, and one detail of the person themself if there is one worth
        // naming. The detail comes off the generated traveller, so it is true
        // of this particular stranger rather than of their kind.
        let mut notice = format!(
            "Somebody is on the old road, coming down towards the court. {}",
            party.sighting
        );
        if let Some(detail) = party.notable() {
            notice.push(' ');
            notice.push_str(detail);
        }
        // Which story this is, when somebody is looking at stories rather than
        // playing. Guessing it from the first sentence is exactly the work the
        // switch exists to save. Both are silent unless the switch is set, so
        // an ordinary game never sees either.
        rehearsal::note_arrival(&rehearsal, party);
        notice.insert_str(0, &rehearsal::announce(&rehearsal, party));
        spawn_visitor_bodies(
            &mut commands,
            &asset_server,
            &mut layouts,
            visitors.party.as_ref().expect("a party just arrived"),
        );
        journal.say(notice);
    }
    if visitors.tick_patience(time.delta_secs()) {
        journal.say(
            "You look up and the court is empty. Whoever it was decided a stranger's fire was not worth the risk after all.",
        );
    }

    let Some(party) = visitors.party.as_mut() else {
        return;
    };
    let target = match party.stage {
        VisitStage::Leaving => visitor_road_entry(),
        _ => visitor_waiting_spot(),
    };
    // Bodies are spawned through `Commands`, so on the frame a party arrives the
    // query is still empty. Counting them keeps an empty query from reading as
    // "everybody has arrived" and teleporting the visit to its waiting stage
    // with nobody ever having walked down the road.
    let mut walking = 0_usize;
    let mut still_travelling = 0_usize;
    for (body, mut transform, mut sprite, mut visibility, composing) in &mut bodies {
        let Some(offset) = party
            .profile()
            .bodies
            .get(body.index)
            .map(|authored| authored.offset)
        else {
            continue;
        };
        let goal = target + offset;
        let here = transform.translation.truncate();
        let step = VISITOR_SPEED * time.delta_secs();
        let to_go = goal - here;
        if party.stage.is_walking() && to_go.length() > step {
            let moved = here + to_go.normalize_or_zero() * step;
            transform.translation.x = moved.x;
            transform.translation.y = moved.y;
            transform.translation.z = exterior_depth(moved.y + PLAYER_GROUND_OFFSET_Y);
            still_travelling += 1;
            if let Some(atlas) = sprite.texture_atlas.as_mut() {
                let facing = if to_go.x.abs() > to_go.y.abs() {
                    if to_go.x < 0.0 {
                        Facing::Left
                    } else {
                        Facing::Right
                    }
                } else if to_go.y > 0.0 {
                    Facing::Up
                } else {
                    Facing::Down
                };
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let frame = ((time.elapsed_secs() / SCRIBE_WALK_SECONDS_PER_FRAME) as usize)
                    % SCRIBE_WALK_FRAMES;
                atlas.index = facing.visitor_walk_row() * npc_art::COLUMNS as usize + frame;
            }
        } else if !party.stage.is_walking() {
            // Standing still, facing out into the court rather than at the wall:
            // this is somebody waiting to be met, not somebody about to knock.
            if let Some(atlas) = sprite.texture_atlas.as_mut() {
                atlas.index = Facing::Down.visitor_walk_row() * npc_art::COLUMNS as usize;
            }
        }
        // A body whose sheets are still loading is not drawn at all. The
        // alternative is watching a stranger walk halfway down the slope in
        // somebody else's face and then change into their own.
        *visibility = if party.stage.is_present() && !composing {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        walking += 1;
    }
    if walking == 0 || still_travelling > 0 {
        return;
    }
    match party.stage {
        VisitStage::Approaching => party.stage = VisitStage::Waiting,
        VisitStage::Leaving => party.gone = true,
        _ => {}
    }
}

/// Clears away a party that has walked off the map.
fn retire_visitors(
    mut commands: Commands,
    mut visitors: ResMut<Visitors>,
    bodies: Query<Entity, With<VisitorBody>>,
) {
    if !visitors.party.as_ref().is_some_and(|party| party.gone) {
        return;
    }
    for entity in &bodies {
        commands.entity(entity).despawn();
    }
    visitors.clear_departed();
}

/// Puts an arriving party on the road.
///
/// Each body is spawned wearing its profile's hand-made sheet and asked to
/// compose its own. The fallback is not decoration: a build with no LPC
/// checkout has no generated art at all, and a visitor who cannot be drawn
/// should still be a visitor. Bodies stay hidden until the composite lands, so
/// nobody watches a stranger change face halfway down the slope.
fn spawn_visitor_bodies(
    commands: &mut Commands,
    asset_server: &AssetServer,
    layouts: &mut Assets<TextureAtlasLayout>,
    party: &visitors::Party,
) {
    let layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(npc_art::FRAME),
        npc_art::COLUMNS,
        npc_art::ROWS,
        None,
        None,
    ));
    let entry = visitor_road_entry();
    for (index, body) in party.profile().bodies.iter().enumerate() {
        let position = entry + body.offset;
        let mut spawned = commands.spawn((
            Sprite {
                image: asset_server.load(body.art),
                texture_atlas: Some(TextureAtlas {
                    layout: layout.clone(),
                    index: Facing::Right.visitor_walk_row() * npc_art::COLUMNS as usize,
                }),
                ..default()
            },
            Transform::from_xyz(
                position.x,
                position.y,
                exterior_depth(position.y + PLAYER_GROUND_OFFSET_Y),
            ),
            ExteriorYSort {
                ground_offset_y: PLAYER_GROUND_OFFSET_Y,
                depth_bias: 0.0,
            },
            VisitorBody { index },
            Interactable {
                kind: InteractableKind::Visitor,
                consumed: false,
            },
        ));
        if let Some(npc) = party.people.get(index) {
            spawned.insert(npc_art::ComposingBody::request(npc, asset_server));
        }
    }
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_ui(
    journal: Res<Journal>,
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
        ),
    >,
    mut prompt: Query<
        &mut TextSpan,
        (
            With<PromptText>,
            Without<StatusText>,
            Without<OverlayTitle>,
            Without<OverlayBody>,
        ),
    >,
    mut overlay: OverlayWidgets,
) {
    // There is no objective line. Working out what a ruin needs is the game, and
    // a corner of the screen telling the player the answer would be the game
    // playing itself. All this says is the date and the last thing that happened.
    if let Ok(mut text) = status.single_mut() {
        let when = ui_knowledge.clock.describe();
        **text = journal.notice.as_ref().map_or_else(
            || format!("THE SCRIBE\n{when}"),
            |notice| format!("THE SCRIBE\n{when}\n\n{notice}"),
        );
    }

    if let Ok(mut text) = progress_text.single_mut() {
        let supplies = progression.supplies_summary();
        let mut knowledge = Vec::new();
        if ui_knowledge.motel_access.keys_found {
            knowledge.push("Numbered motel keys — office");
        }
        let read = ui_knowledge.readings.count();
        let read_line;
        if bible_found(&ui_knowledge.interior_state) {
            read_line = if read <= 1 {
                "Old Gideon Bible — room 3".to_owned()
            } else {
                format!("Old Gideon Bible — room 3 ({read} passages read)")
            };
            knowledge.push(read_line.as_str());
        }
        let knowledge = if knowledge.is_empty() {
            String::new()
        } else {
            format!("\n\nKNOWN\n{}", knowledge.join("\n"))
        };
        // The garden only appears once there is a garden; before the first bed
        // is broken open there is nothing here but ash, and saying so would be
        // one more thing the valley has not given yet.
        let broken = ui_knowledge.garden.broken_ground();
        let garden_line = if broken == 0 {
            String::new()
        } else {
            format!(
                "\n\nGARDEN\n{broken} of {} bays out of the asphalt",
                ui_knowledge.beds.0
            )
        };
        // The block-cutting only shows up once there is a block. Before the
        // first night's work it is not a thing the Scribe does yet.
        let prints_line = if ui_knowledge.collection.made().is_empty() {
            String::new()
        } else {
            format!("\n\nPRINTS\n{}", ui_knowledge.collection.describe())
        };
        // The hearth only starts reporting itself once there is a fire to
        // report. Before that it is one more thing the valley has not given.
        let hearth_line = if hearth_is_lit(&ui_knowledge.interior_state)
            || ui_knowledge.upkeep.pot().bowls() > 0
        {
            let tally = ui_knowledge
                .upkeep
                .tally()
                .map_or_else(String::new, |count| format!("\n{count}"));
            format!(
                "\n\nHEARTH\n{}{tally}",
                ui_knowledge
                    .upkeep
                    .summary(hearth_is_lit(&ui_knowledge.interior_state))
            )
        } else {
            String::new()
        };
        **text = format!(
            "RESTORATION\n{}\n\nTOOLS\n{}\n\nSUPPLIES\n{}{hearth_line}{garden_line}{prints_line}{knowledge}",
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
            || carried_tool_prompt(&progression),
            |(entity, item)| {
                // A bed changes what it wants as it goes, so it is asked first;
                // the cached `TaskTarget` line every other station uses would be
                // stale for five of its six states.
                if let Ok(plot) = ui_knowledge.garden_plots.get(entity) {
                    return garden_plot_prompt(plot, &ui_knowledge.garden, &progression);
                }
                if ui_knowledge.rain_cisterns.get(entity).is_ok() {
                    return cistern_prompt(&ui_knowledge.interior_state, &progression);
                }
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
                if item.kind == InteractableKind::Hearth {
                    return hearth_prompt(
                        &ui_knowledge.interior_state,
                        &progression,
                        &ui_knowledge.upkeep,
                    );
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
                                // Mending it where it lies asks exactly what
                                // mending it out of the pack asks, so the
                                // prompt says so in the same breath.
                                format!(
                                    "E — take the broken {label}     R — repair it     [{}]",
                                    progression::TaskSpec::for_tool_repair(record.tool)
                                        .requirements_text()
                                )
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
                    InteractableKind::Desk => "E — search the old desk     R — repair it",
                    InteractableKind::BibleNightstand => "E — search here",
                    InteractableKind::Visitor => visitor_prompt(&ui_knowledge.visitors),
                    InteractableKind::Bed => "E — sleep",
                    InteractableKind::Salvage => "E — turn this out",
                    InteractableKind::MotelDoor => "Walk through the motel door",
                    InteractableKind::InteriorExit => "Walk onto the exit to step outside",
                    InteractableKind::InteriorRepairable => "R — restore this part of the room",
                    InteractableKind::ExteriorRepairable => "R — restore this part of the building",
                    InteractableKind::Tree => "R — chop this tree",
                    InteractableKind::Sawbuck => "R — saw a log into planks",
                    InteractableKind::StoneOutcrop => "R — work stone out of this outcrop",
                    InteractableKind::Forage => "E — gather what is growing here",
                    InteractableKind::SeedStore => "E — search the seed shelf",
                    InteractableKind::GardenPlot => "R — work this bed",
                    InteractableKind::RainCistern => "R — see to the rain butt",
                }
                .to_owned()
            },
        );
    if let Ok(mut text) = prompt.single_mut() {
        **text = nearby_prompt;
    }

    let visit = ui_knowledge.visitors.party.as_ref();
    let overlay_content = visit.and_then(|party| {
        visit_overlay(
            party,
            &ui_knowledge.collection,
            &progression,
            &ui_knowledge.upkeep,
            &ui_knowledge.motel_access,
        )
    });
    if let Ok(mut visibility) = overlay.root.single_mut() {
        *visibility = if overlay_content.is_some() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if let Ok(mut visibility) = overlay.prompt_panel.single_mut() {
        *visibility = if overlay_content.is_some() {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
    if let Some((heading, content)) = overlay_content {
        if let Ok(mut text) = overlay.title.single_mut() {
            **text = heading;
        }
        if let Ok(mut text) = overlay.body.single_mut() {
            **text = content;
        }
    }
    if let Ok((mut image, mut node)) = overlay.card_art.single_mut() {
        let showing = visit.and_then(|party| match party.stage {
            VisitStage::Choosing => ui_knowledge
                .collection
                .on_hand()
                .first()
                .map(|print| print.art_path()),
            VisitStage::Deciding => party
                .given
                .card
                .as_deref()
                .and_then(cards::print)
                .map(cards::Print::art_path),
            _ => None,
        });
        if let Some(path) = showing {
            image.image = asset_server.load(path);
            node.display = Display::Flex;
        } else {
            node.display = Display::None;
        }
    }
}

/// The screen a visit puts in front of the player. Only the conversation states
/// draw one; a party walking in or standing in the court leaves the world alone.
fn visit_overlay(
    party: &visitors::Party,
    collection: &Collection,
    progression: &Progression,
    state: &upkeep::Upkeep,
    motel_access: &MotelAccess,
) -> Option<(String, String)> {
    match party.stage {
        VisitStage::Telling => {
            let lines = party.spoken();
            let line = lines.get(party.line.min(lines.len().saturating_sub(1)))?;
            Some((
                party.address(),
                format!("\u{201c}{line}\u{201d}\n\nSPACE — listen"),
            ))
        }
        VisitStage::Listening => Some((
            "The Scribe Listens".to_owned(),
            "Their words settle beside what you have been reading in room three. You go looking for the need underneath them…"
                .to_owned(),
        )),
        VisitStage::Deciding => Some(deciding_overlay(
            party,
            collection,
            progression,
            state,
            motel_access,
        )),
        VisitStage::Choosing => Some(choosing_overlay(party, collection)),
        _ => None,
    }
}

/// The name of whoever put the passage into the traveler's language, when that
/// was somebody other than the edition this game is written around.
///
/// The Berean Standard Bible is in the public domain and asks for nothing, so
/// naming it would be machinery on the screen for no one's benefit. A traveler
/// who arrives speaking Spanish is handed a different committee's work, under
/// terms that are theirs and not ours, and their name goes with their words.
fn translation_credit(version: &str) -> String {
    if version.is_empty() || version == DEFAULT_BIBLE_ABBREVIATION {
        return String::new();
    }
    format!(" \u{b7} {version}")
}

/// What the Scribe could do, phrased as things they have rather than things they
/// must. Every line is either an offer they can make or a plain statement of why
/// they cannot, and letting the visitor go is always on the list.
fn deciding_overlay(
    party: &visitors::Party,
    collection: &Collection,
    progression: &Progression,
    state: &upkeep::Upkeep,
    motel_access: &MotelAccess,
) -> (String, String) {
    let mut lines = Vec::new();
    if let Some(need) = party.need.as_ref() {
        lines.push(format!(
            "{}\n\n\u{201c}{}\u{201d}\n{}{}\n",
            need.reflection,
            need.passage.content,
            need.passage.reference,
            translation_credit(&need.passage.version)
        ));
    }
    lines.push(if party.given.food {
        "1  — shared already".to_owned()
    } else {
        food_offer(state, progression).line(state, progression)
    });
    lines.push(party.given.room.as_ref().map_or_else(
        || {
            offerable_room(motel_access).map_or_else(
                || "2  — every door here is locked and you have no key".to_owned(),
                |(_, label)| format!("2  Offer them {label}"),
            )
        },
        |room| format!("2  — {room} is theirs for the night"),
    ));
    let on_hand = collection.on_hand();
    lines.push(party.given.card.as_ref().map_or_else(
        || card_offer_line(party, collection, on_hand.len()),
        |id| {
            cards::print(id).map_or_else(
                || "3  — given".to_owned(),
                |print| format!("3  — you gave them \u{201c}{}\u{201d}", print.title),
            )
        },
    ));
    lines.push("\nSPACE — let them go".to_owned());
    (party.address(), lines.join("\n"))
}

/// The card line, before anything has been handed over. Naming the one the
/// Scribe reaches for is the whole of what the listening buys; it never removes
/// a choice.
fn card_offer_line(party: &visitors::Party, collection: &Collection, on_hand: usize) -> String {
    if on_hand == 0 {
        return "3  — you have cut nothing yet".to_owned();
    }
    party
        .need
        .as_ref()
        .and_then(|need| collection.suggestion_for(&need.need_id))
        .map_or_else(
            || format!("3  Give them one of your prints ({on_hand})"),
            |print| {
                format!(
                    "3  Give them one of your prints ({on_hand}) — your hand goes to \u{201c}{}\u{201d}",
                    print.title
                )
            },
        )
}

fn choosing_overlay(party: &visitors::Party, collection: &Collection) -> (String, String) {
    let suggestion = party
        .need
        .as_ref()
        .and_then(|need| collection.suggestion_for(&need.need_id))
        .map(|print| print.id.clone());
    let mut lines = vec!["What you have cut, and kept.\n".to_owned()];
    for (index, print) in collection.on_hand().iter().enumerate() {
        let marker = if Some(&print.id) == suggestion.as_ref() {
            "  ·  the one your hand went to"
        } else {
            ""
        };
        lines.push(format!(
            "{}  \u{201c}{}\u{201d} — {}{marker}",
            index + 1,
            print.title,
            print.reference
        ));
    }
    lines.push("\nSPACE — keep them all for now".to_owned());
    ("The Block-Prints".to_owned(), lines.join("\n"))
}

/// `P` opens the folio and, once it is open, the folio has the keyboard. It
/// cannot be opened over a conversation or a popup, and nothing underneath it
/// answers while it is, so leafing through prints never walks the Scribe into
/// a wall or starts a repair.
fn handle_portfolio_input(
    keys: Res<ButtonInput<KeyCode>>,
    popup: Res<NarrativePopup>,
    visitors: Res<Visitors>,
    collection: Res<Collection>,
    mut portfolio: ResMut<Portfolio>,
) {
    let cut = collection.made().len();
    if portfolio.is_open() {
        if keys.just_pressed(KeyCode::KeyP) || keys.just_pressed(KeyCode::Escape) {
            portfolio.open = false;
        } else if keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::KeyD) {
            portfolio.turn_forward(cut);
        } else if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::KeyA) {
            portfolio.turn_back(cut);
        }
        return;
    }
    if popup.is_open() || visitor_holds_the_screen(&visitors) {
        return;
    }
    if keys.just_pressed(KeyCode::KeyP) {
        portfolio.open_at_a_real_leaf(cut);
    }
}

/// Every widget the folio owns. Each text node excludes the other so Bevy can
/// prove the two borrows are disjoint.
#[derive(SystemParam)]
struct PortfolioWidgets<'w, 's> {
    root: Query<'w, 's, &'static mut Visibility, With<PortfolioRoot>>,
    art: Query<'w, 's, (&'static mut ImageNode, &'static mut Node), With<PortfolioArt>>,
    caption: Query<'w, 's, &'static mut Text, (With<PortfolioCaption>, Without<PortfolioTally>)>,
    tally: Query<'w, 's, &'static mut Text, (With<PortfolioTally>, Without<PortfolioCaption>)>,
}

fn sync_portfolio_ui(
    portfolio: Res<Portfolio>,
    collection: Res<Collection>,
    asset_server: Res<AssetServer>,
    mut widgets: PortfolioWidgets,
) {
    if let Ok(mut visibility) = widgets.root.single_mut() {
        *visibility = if portfolio.is_open() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if !portfolio.is_open() {
        return;
    }
    let cut = collection.made();
    let showing = cut.get(portfolio.index).and_then(|id| cards::print(id));
    if let Ok((mut image, mut node)) = widgets.art.single_mut() {
        if let Some(print) = showing {
            image.image = asset_server.load(print.art_path());
            node.display = Display::Flex;
        } else {
            node.display = Display::None;
        }
    }
    if let Ok(mut text) = widgets.caption.single_mut() {
        **text = showing.map_or_else(
            || "The folio is empty. No block has been cut here yet.".to_owned(),
            |print| portfolio_caption(print, collection.was_given(&print.id)),
        );
    }
    if let Ok(mut text) = widgets.tally.single_mut() {
        **text = portfolio_tally(portfolio.index, cut.len());
    }
}

/// What is written beside the leaf. Not the verse: the block itself carries
/// that, cut large enough to read, and setting it twice on one screen is what
/// pushes the folio off the bottom of a short window.
fn portfolio_caption(print: &cards::Print, given: bool) -> String {
    let mut caption = format!("\u{201c}{}\u{201d}\n{}", print.title, print.reference);
    if given {
        caption.push_str("\n\nThis one went out in somebody's coat.");
    }
    caption
}

fn portfolio_tally(index: usize, cut: usize) -> String {
    if cut == 0 {
        return "P / ESC — put the folio away".to_owned();
    }
    let place = format!("{} of {cut}", index + 1);
    if cut == 1 {
        return format!("{place}\nP / ESC — put the folio away");
    }
    format!("{place}\n\u{2190} / \u{2192} — turn a leaf\nP / ESC — put the folio away")
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
    let Some(card) = popup.current.as_ref() else {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::progression::SkillId;

    /// A cleared flue and a full pile, from whichever source.
    fn hearth_ready() -> (InteriorState, Progression) {
        let mut interior_state = InteriorState::default();
        interior_state
            .0
            .insert(OFFICE_CHIMNEY_STATE_KEY.to_owned(), "repaired".to_owned());
        let mut progression = Progression::default();
        progression.add_supply(SupplyId::Kindling, HEARTH_KINDLING);
        (interior_state, progression)
    }

    #[test]
    fn a_cold_hearth_names_everything_it_is_still_waiting_on() {
        let interior_state = InteriorState::default();
        let progression = Progression::default();
        let missing = hearth_blockers(&interior_state, &progression);
        assert_eq!(
            missing,
            vec![
                "a cleared chimney".to_owned(),
                format!("{HEARTH_KINDLING} kindling (you have 0)"),
            ]
        );
        assert!(
            hearth_prompt(&interior_state, &progression, &upkeep::Upkeep::default())
                .contains("still needs")
        );
    }

    #[test]
    fn a_ready_hearth_asks_for_nothing_further() {
        let (interior_state, progression) = hearth_ready();
        assert!(hearth_blockers(&interior_state, &progression).is_empty());
        assert!(
            hearth_prompt(&interior_state, &progression, &upkeep::Upkeep::default())
                .contains("light the hearth")
        );
        assert!(!hearth_is_lit(&interior_state));
    }

    /// A lit hearth is never a finished job. It goes on naming what it wants —
    /// the fire first, then the pot — and falls silent only when there is
    /// genuinely nothing to hand it.
    #[test]
    fn a_lit_hearth_goes_on_asking_for_what_it_needs_next() {
        let (mut interior_state, mut progression) = hearth_ready();
        interior_state
            .0
            .insert(OFFICE_HEARTH_STATE_KEY.to_owned(), "repaired".to_owned());
        assert!(hearth_is_lit(&interior_state));
        let mut state = upkeep::Upkeep::default();
        state.lay_a_fire();

        // The kindling that lit it is still in the pack, so the fire is first.
        assert_eq!(
            hearth_prompt(&interior_state, &progression, &state),
            "E — feed the fire     [3 kindling]"
        );
        state.feed_the_fire(&mut progression, false);

        // Fire seen to; the pot is the next thing a person would do.
        progression.add_supply(SupplyId::Ration, 1);
        assert_eq!(
            hearth_prompt(&interior_state, &progression, &state),
            "E — put a ration in the pot     [1 ration]"
        );
        state.add_a_ration(&mut progression);

        // And once there is something in it, water goes further than nothing.
        progression.add_supply(SupplyId::Water, 1);
        assert_eq!(
            hearth_prompt(&interior_state, &progression, &state),
            "E — let the pot down with water     [1 canful of water]"
        );
        state.stretch_the_pot(&mut progression);

        assert_eq!(
            hearth_prompt(&interior_state, &progression, &state),
            "E — tend the hearth",
            "with empty hands and a fed fire it stops asking"
        );
    }

    /// The one case where the fire has to speak up: burning down, and nothing in
    /// the pack to put on it. That is the same shape a missing plank uses.
    #[test]
    fn a_fire_burning_down_with_no_wood_says_so_on_the_hearth() {
        let mut interior_state = InteriorState::default();
        interior_state
            .0
            .insert(OFFICE_HEARTH_STATE_KEY.to_owned(), "repaired".to_owned());
        let state = upkeep::Upkeep::default();
        assert_eq!(state.banked_nights(), 0);
        assert_eq!(
            hearth_prompt(&interior_state, &Progression::default(), &state),
            format!("E — tend the hearth     [still needs {WOOD_FOR_A_NIGHT}]")
        );
    }

    /// The office has walls to mend, and mending all of them is what makes the
    /// fire cheaper. If the scene ever stops carrying wall sections under this
    /// prefix the dividend would silently pay itself from the first day.
    #[test]
    fn the_office_walls_are_a_real_repair_that_has_to_be_finished() {
        let office = interior::InteriorMap::load(interior::InteriorId::Office);
        let walls = office
            .mutable_elements()
            .iter()
            .filter(|element| element.id.starts_with(OFFICE_WALL_PREFIX))
            .map(|element| format!("{}/{}", office.id(), element.id))
            .collect::<Vec<_>>();
        assert!(walls.len() > 1, "the office should be a room, not a wall");

        let mut interior_state = InteriorState::default();
        assert!(!walls_hold_the_heat(&office, &interior_state));
        for (index, key) in walls.iter().enumerate() {
            interior_state.0.insert(key.clone(), "repaired".to_owned());
            assert_eq!(
                walls_hold_the_heat(&office, &interior_state),
                index + 1 == walls.len(),
                "the last wall is what finishes the room"
            );
        }
    }

    /// Deadfall keeps falling and plants keep growing; a felled tree stays
    /// felled and a quarried outcrop stays quarried. If this ever went the other
    /// way the valley would either strip bare for good or stop being finite.
    #[test]
    fn only_the_things_that_grow_again_come_back() {
        for kind in [
            InteractableKind::Forage,
            InteractableKind::Kindling,
            InteractableKind::Log,
        ] {
            assert!(regrows(kind));
        }
        for kind in [
            InteractableKind::Plank,
            InteractableKind::Tree,
            InteractableKind::StoneOutcrop,
            InteractableKind::SeedStore,
            InteractableKind::Salvage,
            InteractableKind::Tool,
        ] {
            assert!(!regrows(kind));
        }
    }

    /// The offer on screen and the key that takes it read the same list, so this
    /// walks the whole ladder down: a full pot, one bowl left, the pack, the last
    /// of the pack, nothing.
    #[test]
    fn the_food_offer_says_exactly_what_giving_it_would_cost() {
        let mut state = upkeep::Upkeep::default();
        let mut progression = Progression::default();
        assert_eq!(food_offer(&state, &progression), FoodOffer::Nothing);
        assert!(food_offer(&state, &progression)
            .line(&state, &progression)
            .contains("nothing to eat yourself"));

        progression.add_supply(SupplyId::Ration, 1);
        assert_eq!(food_offer(&state, &progression), FoodOffer::LastRation);
        assert!(food_offer(&state, &progression)
            .line(&state, &progression)
            .contains("half each"));

        progression.add_supply(SupplyId::Ration, 2);
        assert_eq!(food_offer(&state, &progression), FoodOffer::Ration);

        // A pot with something in it is always reached for before the pack.
        state.add_a_ration(&mut progression);
        assert_eq!(food_offer(&state, &progression), FoodOffer::Bowl);
        assert!(food_offer(&state, &progression)
            .line(&state, &progression)
            .contains("2 in the pot"));

        state.ladle_a_bowl();
        assert_eq!(food_offer(&state, &progression), FoodOffer::LastBowl);
        assert!(food_offer(&state, &progression)
            .line(&state, &progression)
            .contains("half each"));
    }

    #[test]
    fn every_vignette_a_visitor_can_tell_has_a_reviewed_fallback() {
        for item in waystation_shared::vignettes() {
            assert!(fixture_response(&item.id).is_some());
        }
    }

    #[test]
    fn save_data_keeps_mutable_room_state_by_stable_id() {
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
        let mut garden = Garden::default();
        garden.advance("garden-plot-00");
        let save = SaveData::capture(
            &interior_state,
            &motel_access,
            &progression,
            &garden,
            Clock::default(),
            &Visitors::default(),
            &Collection::default(),
            &Readings::default(),
            &Salvaged::default(),
            &upkeep::Upkeep::default(),
        );

        assert_eq!(save.version, 9);
        assert_eq!(save.garden.stage("garden-plot-00"), PlotStage::Fallow);
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
            false,
            true,
        ));
        assert!(!interaction_key_matches(
            InteractableKind::InteriorRepairable,
            true,
            false,
        ));
        assert!(interaction_key_matches(
            InteractableKind::BibleNightstand,
            true,
            false,
        ));
        assert!(!interaction_key_matches(
            InteractableKind::BibleNightstand,
            false,
            true,
        ));
        // A bed and a stranger both answer to E, never to the work key.
        assert!(interaction_key_matches(InteractableKind::Bed, true, false));
        assert!(!interaction_key_matches(InteractableKind::Bed, false, true));
        assert!(interaction_key_matches(
            InteractableKind::Visitor,
            true,
            false
        ));
        // The desk can be searched or mended, so it takes either.
        assert!(interaction_key_matches(InteractableKind::Desk, false, true));
        assert!(interaction_key_matches(InteractableKind::Desk, true, false));
    }

    #[test]
    fn finding_the_room_three_bible_is_remembered_across_a_save() {
        let mut interior_state = InteriorState::default();
        assert!(!bible_found(&interior_state));
        record_bible_discovery(&mut interior_state);
        assert!(bible_found(&interior_state));
    }

    #[test]
    fn a_cold_hearth_says_what_is_wrong_with_it_and_never_where_to_go() {
        let nothing = hearth_blockers(&InteriorState::default(), &Progression::default());
        let complaint = hearth_complaint(&nothing);
        assert!(
            complaint.contains("chimney"),
            "the Scribe should name the flue: {complaint}"
        );
        assert!(
            complaint.contains("catch"),
            "the Scribe should notice there is no dry fuel: {complaint}"
        );

        let (interior_state, progression) = hearth_ready();
        assert!(
            hearth_blockers(&interior_state, &progression).is_empty(),
            "a cleared flue and a full pile leave nothing to complain about"
        );

        // Nothing in any complaint tells the player where to solve it. Finding
        // the ladder, the roof, and the deadfall is the game.
        let mut kindling_only = Progression::default();
        kindling_only.add_supply(SupplyId::Kindling, HEARTH_KINDLING);
        for missing in [
            hearth_blockers(&InteriorState::default(), &Progression::default()),
            hearth_blockers(&InteriorState::default(), &kindling_only),
            hearth_blockers(&interior_state, &Progression::default()),
        ] {
            let complaint = hearth_complaint(&missing);
            for direction in ["roof", "ladder", "beneath the old growth", "Requires"] {
                assert!(
                    !complaint.contains(direction),
                    "the hearth is giving directions: {complaint}"
                );
            }
        }
    }

    /// A tool taken out of the shed goes where the Scribe goes, and so does the
    /// job of mending it. Without this line the only place R ever worked on a
    /// broken tool was the tile it was found on, and picking one up locked the
    /// player out of repairing it.
    #[test]
    fn a_broken_tool_in_the_pack_says_which_key_works_on_it() {
        let mut progression = Progression::default();
        assert!(
            carried_tool_prompt(&progression).contains("WASD"),
            "an empty pack leaves the controls line alone"
        );

        progression.register_tool_instance("pickaxe-01", ToolId::Pickaxe, ToolCondition::Broken);
        assert!(
            carried_tool_prompt(&progression).contains("WASD"),
            "a tool still on the shed floor is the shed's business, not the pack's"
        );

        progression.pick_up_tool("pickaxe-01").expect("carried");
        let prompt = carried_tool_prompt(&progression);
        assert!(
            prompt.starts_with("R — repair the broken pickaxe"),
            "{prompt}"
        );
        // Like every other station, it names what the work wants and stops
        // short of saying where to find it.
        assert!(prompt.contains("Upkeep 2"), "{prompt}");
        assert!(prompt.contains("hammer"), "{prompt}");
        assert!(prompt.contains("sound plank or 1 fallen log"), "{prompt}");
        for direction in ["shed", "tree", "sawbuck"] {
            assert!(
                !prompt.contains(direction),
                "the prompt is giving directions: {prompt}"
            );
        }

        progression.set_tool_condition("pickaxe-01", ToolCondition::Serviceable);
        assert!(carried_tool_prompt(&progression).contains("WASD"));
    }

    /// A broken tool the valley cannot mend is a dead end that looks exactly
    /// like a puzzle. Every tool the shed authors broken has to be reachable
    /// from tools the shed also authors sound.
    #[test]
    fn every_tool_that_starts_broken_can_be_put_back_into_service() {
        let shed = interior::InteriorMap::load(interior::InteriorId::ToolShed);
        let items = shed.portable_items();
        let broken = items
            .iter()
            .filter(|item| item.condition == ToolCondition::Broken)
            .collect::<Vec<_>>();
        assert!(
            !broken.is_empty(),
            "a shed with nothing to mend makes the whole repair path unreachable"
        );
        for item in broken {
            for wanted in progression::TaskSpec::for_tool_repair(item.tool).tools {
                assert!(
                    items.iter().any(|other| other.tool == wanted
                        && other.condition == ToolCondition::Serviceable),
                    "{} needs a {} the valley never hands over sound",
                    item.id,
                    wanted.label()
                );
            }
        }
    }

    #[test]
    fn a_room_cannot_be_offered_before_the_keys_are_found() {
        assert!(offerable_room(&MotelAccess::default()).is_none());
        let (interior_id, label) =
            offerable_room(&MotelAccess { keys_found: true }).expect("a room to offer");
        assert_eq!(interior_id, interior::InteriorId::Room01);
        assert!(!label.is_empty());
    }

    #[test]
    fn the_farewell_reports_only_what_was_actually_given() {
        let mut party = visitors::Visitors::default()
            .arrive(&mut Chance::default(), visitors::CURRENT_ERA)
            .clone();
        party.names = vec!["Mara".to_owned()];

        let empty_handed = farewell_notice(&party);
        assert!(empty_handed.contains("Mara"));
        assert!(
            empty_handed.contains("did not"),
            "turning somebody away should be stated plainly, not scolded: {empty_handed}"
        );

        party.given.food = true;
        party.given.room = Some("room one".to_owned());
        let generous = farewell_notice(&party);
        assert!(generous.contains("fed"), "{generous}");
        assert!(generous.contains("housed"), "{generous}");
        assert!(
            !generous.contains("did not"),
            "a fed and housed guest was still described as turned away: {generous}"
        );
    }

    /// A party parked in one stage, for exercising the screens.
    fn party_at(stage: VisitStage) -> visitors::Party {
        let mut party = visitors::Visitors::default()
            .arrive(&mut Chance::default(), visitors::CURRENT_ERA)
            .clone();
        party.stage = stage;
        party.names = vec!["Mara".to_owned()];
        party
    }

    #[test]
    fn only_a_conversation_takes_the_screen_away_from_the_valley() {
        let collection = Collection::default();
        let progression = Progression::default();
        let access = MotelAccess::default();
        for stage in [
            VisitStage::Approaching,
            VisitStage::Waiting,
            VisitStage::Lodging,
            VisitStage::Leaving,
        ] {
            assert!(
                visit_overlay(
                    &party_at(stage),
                    &collection,
                    &progression,
                    &upkeep::Upkeep::default(),
                    &access
                )
                .is_none(),
                "{stage:?} should leave the player looking at the world"
            );
            assert!(
                !stage.holds_the_screen(),
                "{stage:?} should not lock movement"
            );
        }
        for stage in [
            VisitStage::Telling,
            VisitStage::Listening,
            VisitStage::Deciding,
            VisitStage::Choosing,
        ] {
            assert!(
                visit_overlay(
                    &party_at(stage),
                    &collection,
                    &progression,
                    &upkeep::Upkeep::default(),
                    &access
                )
                .is_some(),
                "{stage:?} needs something on screen"
            );
            assert!(stage.holds_the_screen(), "{stage:?} should hold the screen");
        }
    }

    #[test]
    fn the_choosing_screen_always_offers_a_way_out_of_giving_anything() {
        let party = party_at(VisitStage::Deciding);
        let (_, body) = deciding_overlay(
            &party,
            &Collection::default(),
            &Progression::default(),
            &upkeep::Upkeep::default(),
            &MotelAccess::default(),
        );
        assert!(
            body.contains("SPACE"),
            "there must always be a way to simply let somebody go: {body}"
        );
        // With nothing in the pack, nothing locked, and nothing cut, every offer
        // reads as a plain statement of what the Scribe does not have.
        assert!(body.contains("nothing to eat"), "{body}");
        assert!(body.contains("locked"), "{body}");
        assert!(body.contains("cut nothing"), "{body}");
    }

    #[test]
    fn the_folio_wraps_in_both_directions_so_no_leaf_is_a_dead_end() {
        let mut folio = Portfolio::default();
        folio.turn_back(3);
        assert_eq!(folio.index, 2, "turning back from the first leaf");
        folio.turn_forward(3);
        assert_eq!(folio.index, 0, "turning past the last leaf");
        folio.turn_forward(1);
        assert_eq!(folio.index, 0, "a folio of one has nowhere else to go");
    }

    #[test]
    fn an_empty_folio_answers_the_key_without_moving_anywhere() {
        let mut folio = Portfolio::default();
        folio.turn_forward(0);
        folio.turn_back(0);
        assert_eq!(folio.index, 0);
        assert!(
            !portfolio_tally(0, 0).contains("of 0"),
            "no leaf is counted"
        );
    }

    #[test]
    fn a_folio_that_grew_while_shut_still_opens_at_a_leaf_that_exists() {
        let mut folio = Portfolio::default();
        folio.open_at_a_real_leaf(4);
        folio.turn_back(4);
        assert_eq!(folio.index, 3);
        folio.open = false;
        // Prints are never un-cut, but the catalogue can lose an entry between
        // saves, and an index past the end would draw nothing at all.
        folio.open_at_a_real_leaf(2);
        assert!(folio.is_open());
        assert_eq!(folio.index, 1);
    }

    #[test]
    fn a_print_already_carried_away_is_still_in_the_folio_and_says_so() {
        let print = cards::prints().first().expect("a catalogue entry");
        let mut collection = Collection::default();
        collection.restore(
            vec![print.id.clone()],
            vec![print.id.clone()],
            cards::Tier::Monochrome,
        );

        assert!(
            collection.has(&print.id),
            "giving it away did not un-cut it"
        );
        assert!(collection.was_given(&print.id));
        let caption = portfolio_caption(print, collection.was_given(&print.id));
        assert!(caption.contains(&print.reference), "the leaf is named");
        assert!(
            !caption.contains(&print.verse),
            "the block already carries the verse; setting it twice overruns the panel"
        );
        assert!(caption.contains("went out in somebody's coat"));
        assert!(
            !portfolio_caption(print, false).contains("went out"),
            "a print still on hand must not be described as gone"
        );
    }

    #[test]
    fn the_folio_names_which_leaf_of_how_many_is_open() {
        assert!(portfolio_tally(2, 7).starts_with("3 of 7"));
        assert!(
            !portfolio_tally(0, 1).contains('\u{2192}'),
            "a single leaf offers no turning"
        );
        assert!(portfolio_tally(0, 2).contains('\u{2192}'));
    }

    #[test]
    fn the_scribe_reaches_for_a_card_without_ever_removing_the_choice() {
        let mut collection = Collection::default();
        let mut chance = Chance::default();
        while collection.cut_a_block(&mut chance, None).is_some() {}
        let mut party = party_at(VisitStage::Deciding);
        party.need = Some(fixture_response("mara_grief").expect("a reviewed fixture"));

        let offer = card_offer_line(&party, &collection, collection.on_hand().len());
        assert!(
            offer.starts_with('3'),
            "the card offer keeps its number: {offer}"
        );
        let (_, body) = choosing_overlay(&party, &collection);
        for (index, print) in collection.on_hand().iter().enumerate() {
            assert!(
                body.contains(&print.title),
                "{} is on hand but not listed",
                print.id
            );
            assert!(body.contains(&format!("{}  ", index + 1)));
        }
        assert!(
            body.contains("SPACE"),
            "keeping every card must stay possible: {body}"
        );
    }

    /// The game is the game. A traveler standing in the court is a person with
    /// something to carry, and the screen has no business naming the machinery
    /// that chose their passage — not the model, not the routing, not the
    /// catalogue it came from. Whoever wants to know where the words came from
    /// can read the repository; the Scribe just hands over a card.
    ///
    /// A translation that is not the public-domain one is the single exception,
    /// and it is a credit rather than machinery — see
    /// `a_translation_that_is_not_ours_to_give_is_named_on_the_card`.
    #[test]
    fn no_screen_a_traveler_puts_up_ever_names_the_technology_behind_it() {
        const MACHINERY: [&str; 7] = [
            "Gloo",
            "YouVersion",
            "BSB",
            "fixture",
            "AI",
            "catalogue",
            "model",
        ];
        let stages = [
            VisitStage::Approaching,
            VisitStage::Waiting,
            VisitStage::Telling,
            VisitStage::Listening,
            VisitStage::Deciding,
            VisitStage::Choosing,
            VisitStage::Lodging,
            VisitStage::Leaving,
        ];
        for stage in stages {
            let mut party = party_at(stage);
            party.need = Some(fixture_response("mara_grief").expect("a reviewed fixture"));
            let Some((heading, body)) = visit_overlay(
                &party,
                &Collection::default(),
                &Progression::default(),
                &upkeep::Upkeep::default(),
                &MotelAccess::default(),
            ) else {
                continue;
            };
            let shown = format!("{heading}\n{body}");
            for word in MACHINERY {
                assert!(
                    !shown.contains(word),
                    "{stage:?} says {word:?} to the player:\n{shown}"
                );
            }
        }
    }

    /// With a `YouVersion` key the passage arrives in the traveler's own
    /// language, from a committee whose edition is not in the public domain the
    /// way the Berean Standard Bible is. Those words are lent, not given, and
    /// the card says whose they are. English asks for nothing and is told
    /// nothing, so the demo reads exactly as it always has.
    #[test]
    fn a_translation_that_is_not_ours_to_give_is_named_on_the_card() {
        let shown = |version: &str| {
            let mut party = party_at(VisitStage::Deciding);
            let mut need = fixture_response("mara_grief").expect("a reviewed fixture");
            need.passage.version = version.to_owned();
            party.need = Some(need);
            let (_, body) = deciding_overlay(
                &party,
                &Collection::default(),
                &Progression::default(),
                &upkeep::Upkeep::default(),
                &MotelAccess::default(),
            );
            body
        };

        assert!(shown("NVI").contains("Psalm 34:18 \u{b7} NVI"));
        assert!(shown(DEFAULT_BIBLE_ABBREVIATION).contains("Psalm 34:18\n"));
        assert!(!shown(DEFAULT_BIBLE_ABBREVIATION).contains('\u{b7}'));
        // A server that sends no version at all must not print a bare separator.
        assert!(!shown("").contains('\u{b7}'));
    }

    #[test]
    fn every_profile_a_visitor_can_arrive_as_has_art_in_the_runtime_tree() {
        for profile in &visitors::PROFILES {
            for body in profile.bodies {
                let path = std::path::Path::new("runtime-assets").join(body.art);
                assert!(
                    path.is_file(),
                    "{} needs {} built; run `make assets`",
                    profile.id,
                    body.art
                );
            }
        }
    }

    #[test]
    fn every_print_the_scribe_can_cut_has_a_card_in_the_runtime_tree() {
        for print in cards::prints() {
            let path = std::path::Path::new("runtime-assets").join(print.art_path());
            assert!(
                path.is_file(),
                "{} has no composed card; run `make assets`",
                print.id
            );
        }
    }

    #[test]
    fn every_authored_interaction_can_actually_be_stood_next_to() {
        // A search rectangle behind a wall, inside a bed, or in the middle of a
        // collision block looks perfectly correct in the JSON and is simply
        // unreachable in play. Nothing else would catch it: the scene loads, the
        // entity spawns, and the player can never get close enough to notice.
        for interior_id in interior::InteriorId::ALL {
            let room = interior::InteriorMap::load(interior_id);
            for interaction in room.interactions() {
                // Proximity is measured to a rectangle's centre, so the test
                // has to ask the real question: is there anywhere at all inside
                // that radius the Scribe can put their feet?
                let reachable = (1_u8..=9).any(|ring| {
                    (0_u8..24).any(|step| {
                        #[allow(clippy::cast_precision_loss)]
                        let angle = f32::from(step) * std::f32::consts::TAU / 24.0;
                        #[allow(clippy::cast_precision_loss)]
                        let reach = f32::from(ring) * INTERACT_DISTANCE / 9.0;
                        let stand = interaction.center + Vec2::from_angle(angle) * reach;
                        room.is_area_walkable(
                            stand + PLAYER_COLLISION_OFFSET,
                            PLAYER_COLLISION_SIZE,
                        )
                    })
                });
                assert!(
                    reachable,
                    "{}/{} is authored where the Scribe can never stand beside it",
                    room.id(),
                    interaction.id
                );
            }
        }
    }

    /// Standing beside a station is not enough: proximity picks whichever
    /// interactable is *nearest*, and the tool shed is crowded — a shelf, a
    /// crate, and six tools on the floor. A bench in the wrong corner spawns,
    /// draws, and is quietly unselectable, because something smaller is always
    /// half a pace closer. There has to be ground the Scribe can work from.
    #[test]
    fn every_authored_work_station_has_ground_to_be_worked_from() {
        for interior_id in interior::InteriorId::ALL {
            let room = interior::InteriorMap::load(interior_id);
            let rivals: Vec<Vec2> = room
                .interactions()
                .iter()
                .filter(|other| other.kind != interior::SceneInteractionKind::Work)
                .map(|other| other.center)
                .chain(room.portable_items().iter().map(|item| item.center))
                .collect();
            for station in room
                .interactions()
                .iter()
                .filter(|interaction| interaction.kind == interior::SceneInteractionKind::Work)
            {
                let workable = (1_u8..=9).any(|ring| {
                    (0_u8..24).any(|step| {
                        #[allow(clippy::cast_precision_loss)]
                        let angle = f32::from(step) * std::f32::consts::TAU / 24.0;
                        #[allow(clippy::cast_precision_loss)]
                        let reach = f32::from(ring) * INTERACT_DISTANCE / 9.0;
                        let stand = station.center + Vec2::from_angle(angle) * reach;
                        // In front of the bench, not on it and not behind it. A
                        // spot inside the rectangle always wins on distance and
                        // proves nothing; a spot behind one standing against the
                        // back wall means working it from inside the wall.
                        let in_front = stand.y < station.center.y - station.size.y / 2.0;
                        in_front
                            && room.is_area_walkable(
                                stand + PLAYER_COLLISION_OFFSET,
                                PLAYER_COLLISION_SIZE,
                            )
                            && rivals.iter().all(|rival| {
                                stand.distance(*rival) > stand.distance(station.center)
                            })
                    })
                });
                assert!(
                    workable,
                    "{}/{} is authored where something else always answers the key first",
                    room.id(),
                    station.id
                );
            }
        }
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

    /// Work the world spawns rather than the room files author: the standing
    /// stations, one felling per authored tree, and one full season on every
    /// garden bed. Cultivation lives entirely here, so a coverage check that
    /// reads only the scene JSON would call it unreachable and be wrong.
    fn standing_station_tasks() -> Vec<progression::TaskSpec> {
        let mut tasks = vec![
            progression::TaskSpec::for_milling(),
            progression::TaskSpec::for_quarrying(),
            progression::TaskSpec::for_drawing_water(),
        ];
        tasks.extend(TREE_PLACEMENTS.map(|_| progression::TaskSpec::for_tree_chopping()));
        for _ in authored_parking_bays() {
            tasks.extend([
                progression::TaskSpec::for_breaking_ground(),
                progression::TaskSpec::for_tilling(),
                progression::TaskSpec::for_sowing(),
                progression::TaskSpec::for_watering(),
                progression::TaskSpec::for_harvest(),
            ]);
        }
        tasks
    }

    /// Every bay the lot authors, as the game reads it: id, world centre, and
    /// size. The tests go through the same content the editor writes.
    fn authored_parking_bays() -> Vec<(String, Vec2, Vec2)> {
        let parking = interior::MotelParkingMap::load();
        parking
            .mutable_elements()
            .iter()
            .map(|element| {
                let size = element
                    .states
                    .get("damaged")
                    .map_or(GARDEN_PLOT_SIZE, |visual| visual.size);
                (
                    element.id.clone(),
                    parking.element_center(element, size),
                    size,
                )
            })
            .collect()
    }

    /// One authored bay, standing in for its component so the prompt and work
    /// paths can be exercised without a running world.
    fn bay_plot() -> GardenPlot {
        let element = interior::MotelParkingMap::load()
            .mutable_elements()
            .first()
            .cloned()
            .expect("the lot authors at least one bay");
        GardenPlot {
            id: element.id,
            art: String::new(),
            paved: "paved".to_owned(),
            broken: "broken".to_owned(),
            break_task: element.task,
        }
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

    /// The garden fails the same silent way the rest of the economy would, but
    /// worse: a bed that cannot be sown looks identical to one that has not been
    /// sown yet. There is exactly one sack of seed in the valley and no second
    /// source but the garden itself, so the loop has to close on its own.
    #[test]
    fn the_one_sack_of_seed_can_open_a_garden_that_then_keeps_itself() {
        let sowing = progression::TaskSpec::for_sowing();
        let watering = progression::TaskSpec::for_watering();
        let harvest = progression::TaskSpec::for_harvest();
        let drawing = progression::TaskSpec::for_drawing_water();
        let garden_chain = [sowing.clone(), watering, harvest.clone()];

        // The shed shelf has to cover at least the first sowing, or nothing can
        // ever be planted and no amount of yield will help.
        assert!(u32::from(SHED_SEED_STORE) >= total_demand(&[sowing], SupplyId::Seed));
        assert!(
            !authored_parking_bays().is_empty(),
            "the lot authors no beds"
        );
        // After that the beds are the only seed source there is, so a season
        // must hand back more than it took or the garden runs down to nothing.
        assert!(
            total_yield(&garden_chain, SupplyId::Seed)
                > total_demand(&garden_chain, SupplyId::Seed)
        );
        assert!(
            yield_per_task(&drawing, SupplyId::Water)
                >= total_demand(&garden_chain, SupplyId::Water)
        );
        // Something has to feed the Scribe before the first harvest, and forage
        // is the only thing that does.
        assert!(!FORAGE_PLACEMENTS.is_empty());
        const { assert!(FORAGE_RATIONS > 0) };
        // Rations exist to be spent by travellers who have not arrived yet, so
        // nothing authored may quietly depend on them.
        assert_eq!(
            total_demand(&authored_restoration_tasks(), SupplyId::Ration),
            0
        );
        assert!(yield_per_task(&harvest, SupplyId::Ration) > 0);
    }

    /// The valley hands over nothing manufactured. Seed comes off one shed
    /// shelf and then only from the garden; the rain butt is a repair before it
    /// is a water source; and the only thing the Scribe can pick up off the
    /// ground is what grew there.
    #[test]
    fn nothing_manufactured_is_lying_about_the_valley() {
        let shed = interior::InteriorMap::load(interior::InteriorId::ToolShed);
        let seed_shelves = shed
            .interactions()
            .iter()
            .filter(|interaction| interaction.discovery == interior::SceneDiscovery::SeedStore)
            .count();
        assert_eq!(seed_shelves, 1, "seed comes from exactly one place");

        // Forage is food, never seed: a gathered plant cannot short-circuit the
        // garden's own seed loop.
        assert!(FORAGE_PLACEMENTS
            .iter()
            .all(|(_, _, art)| art.starts_with("world/forage_")));

        let mut interior_state = InteriorState::default();
        assert!(!cistern_holds_water(&interior_state));
        let empty = Progression::default();
        assert!(cistern_prompt(&interior_state, &empty).contains("rain butt"));
        interior_state
            .0
            .insert(RAIN_CISTERN_STATE_KEY.to_owned(), "repaired".to_owned());
        assert!(cistern_holds_water(&interior_state));
        assert!(cistern_prompt(&interior_state, &empty).contains("cistern"));
    }

    /// A bed asks for something different five times over, and the one state it
    /// asks for nothing still has to say why.
    #[test]
    fn a_bed_names_what_it_wants_at_every_stage_of_its_season() {
        let mut garden = Garden::default();
        let mut progression = Progression::default();
        let plot = &bay_plot();

        // Cultivation is locked until Upkeep 1, and the prompt says so rather
        // than showing a bare requirement the player cannot read.
        assert!(garden_plot_prompt(plot, &garden, &progression).contains("still needs"));
        for _ in 0..3 {
            progression
                .attempt(&progression::TaskSpec::for_kind("debris"))
                .expect("cleaning");
        }
        assert!(garden_plot_prompt(plot, &garden, &progression).contains("a pickaxe"));
        progression.add_tool(ToolId::Pickaxe);
        progression.add_tool(ToolId::Hoe);
        progression.add_tool(ToolId::WateringCan);

        for expected in ["tear out", "till", "sow", "water"] {
            let prompt = garden_plot_prompt(plot, &garden, &progression);
            assert!(
                prompt.contains(expected),
                "{expected} missing from {prompt}"
            );
            let task = plot.work_task(garden.stage(&plot.id)).expect("work to do");
            progression.attempt(&task).ok();
            garden.advance(&plot.id);
        }

        // Growing offers no key at all, so it must not read like a refusal.
        let waiting = garden_plot_prompt(plot, &garden, &progression);
        assert!(!waiting.starts_with("R —"), "{waiting}");
        assert!(waiting.contains("barely up"), "{waiting}");
        garden.tick(garden::RIPEN_SECONDS);
        assert!(garden_plot_prompt(plot, &garden, &progression).contains("harvest"));
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
        for station in standing_station_tasks() {
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
        let mut tasks = authored_restoration_tasks();
        tasks.extend(standing_station_tasks());
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

    /// The lot is authored in `content/buildings/motel-parking.json` so it can be
    /// laid out in the editor, which means the editor can also walk it into the
    /// motel, into water, or apart from itself. A run with gaps in it is not a
    /// parking lot, and a bay inside a wall cannot be reached.
    #[test]
    fn the_authored_parking_bays_lie_in_one_row_in_front_of_the_motel() {
        let grid = terrain::WorldGrid::generate(terrain::WORLD_SEED);
        let motel = interior::MotelExteriorMap::load();
        let bays = authored_parking_bays();

        assert_eq!(
            bays.len(),
            9,
            "six bays for the rooms and three for the office"
        );
        let mut previous: Option<(Vec2, Vec2)> = None;
        for (id, centre, size) in bays {
            assert!(
                grid.supports_land_footprint(centre, size),
                "bay {id} is standing in water"
            );
            assert!(
                motel.is_area_walkable(centre, size),
                "bay {id} is inside the motel"
            );
            assert!(
                !ExteriorRect::new(centre, size)
                    .overlaps(ExteriorRect::new(MOTEL_SIGN_POSITION, MOTEL_SIGN_SIZE)),
                "bay {id} is laid over the motel sign"
            );
            if let Some((previous_centre, previous_size)) = previous {
                assert!(
                    (centre.y - previous_centre.y).abs() < f32::EPSILON,
                    "bay {id} has come off the line the others sit on"
                );
                assert!(
                    (centre.x - previous_centre.x - previous_size.x).abs() < f32::EPSILON,
                    "bay {id} leaves a gap in the run"
                );
            }
            previous = Some((centre, size));
        }
    }

    /// Content and code both describe what levering a slab up costs: the pair so
    /// the editor can show and change it, `for_breaking_ground` so the coverage
    /// tests can reason about it. They have to agree.
    #[test]
    fn the_authored_bay_task_matches_the_one_the_economy_is_checked_against() {
        let expected = progression::TaskSpec::for_breaking_ground();
        for element in interior::MotelParkingMap::load().mutable_elements() {
            assert_eq!(element.task, expected, "bay {} disagrees", element.id);
        }
    }

    /// The cistern has to stand with the lot rather than off across the valley.
    /// Nine bays is a walk from end to end, and that is the lot being long
    /// rather than the water being misplaced — what matters is that it is on the
    /// same frontage, at one end of the run, and not standing in a bed.
    #[test]
    fn the_cistern_stands_at_the_end_of_the_bays() {
        let grid = terrain::WorldGrid::generate(terrain::WORLD_SEED);
        let motel = interior::MotelExteriorMap::load();
        let tool_shed = interior::ToolShedExteriorMap::load();
        let obstacles = ExteriorObstacles::default();

        let cistern = safe_pickup_position(
            &grid,
            &motel,
            &tool_shed,
            &obstacles,
            &[],
            RAIN_CISTERN_POSITION,
            RAIN_CISTERN_SIZE,
        );
        assert_eq!(
            cistern, RAIN_CISTERN_POSITION,
            "the rain butt slid off its authored spot instead of failing loudly"
        );
        assert!(grid.supports_land_footprint(cistern, RAIN_CISTERN_SIZE));
        assert!(motel.is_area_walkable(cistern, RAIN_CISTERN_SIZE));
        let bays = authored_parking_bays();
        let nearest = bays
            .iter()
            .map(|(_, centre, _)| centre.distance(cistern))
            .fold(f32::INFINITY, f32::min);
        assert!(
            nearest < 200.0,
            "the cistern is {nearest} from the nearest bay"
        );
        for (id, centre, size) in bays {
            assert!(
                (cistern.y - centre.y).abs() < 160.0,
                "the cistern is off the frontage bay {id} sits on"
            );
            assert!(
                !ExteriorRect::new(cistern, RAIN_CISTERN_SIZE)
                    .overlaps(ExteriorRect::new(centre, size)),
                "the cistern is standing in bay {id}"
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
