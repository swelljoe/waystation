//! The Waystation at the Edge of the Ash — hackathon vertical slice.

#![allow(clippy::needless_pass_by_value)]

mod game_audio;
mod interior;
mod progression;
mod terrain;

use std::{
    collections::HashMap,
    fmt::Write as _,
    sync::{Arc, Mutex},
};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use progression::{Progression, SupplyId, TaskAction, ToolId};
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
const DROP_SEARCH_STEP: f32 = terrain::TILE_SIZE;
const DROP_SEARCH_RINGS: i16 = 72;

const TREE_PLACEMENTS: [(f32, f32, f32); 15] = [
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
];

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
                animate_player,
                update_player_tree_occlusion,
                sync_player_occlusion_crown
                    .after(update_exterior_depth)
                    .after(animate_player)
                    .after(update_player_tree_occlusion),
                follow_player,
                terrain::update_debug_overlay,
                update_nearby_interaction,
                handle_interaction,
                handle_story_input,
                poll_interpretation,
                sync_world_state,
                sync_ui,
                save_story,
            )
                .chain(),
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
}

#[derive(Component)]
struct PlayerAnimation {
    timer: Timer,
    facing: Facing,
    frame: usize,
    last_position: Vec2,
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
    Traveler,
    MotelDoor,
    InteriorExit,
    InteriorRepairable,
    ExteriorRepairable,
}

#[derive(Component)]
struct WorldPickup {
    id: String,
    reward: PickupReward,
}

#[derive(Clone, Copy)]
enum PickupReward {
    Supply(SupplyId, u16),
    Tool(ToolId),
}

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
struct MovementEnvironment<'w, 's> {
    location: Res<'w, WorldLocation>,
    interior: Res<'w, interior::InteriorMap>,
    motel: Res<'w, interior::MotelExteriorMap>,
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

#[derive(Resource, Default, Debug)]
struct ExteriorObstacles {
    tree_trunks: Vec<ExteriorRect>,
    tree_art: Vec<ExteriorRect>,
}

impl ExteriorObstacles {
    fn player_can_stand(&self, bounds: ExteriorRect) -> bool {
        self.tree_trunks
            .iter()
            .all(|obstacle| !obstacle.overlaps(bounds))
    }

    fn prop_is_clear(&self, bounds: ExteriorRect) -> bool {
        self.tree_art
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
            version: 4,
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
    interior_state: Res<InteriorState>,
    progression: Res<Progression>,
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

    let interior_map = interior::InteriorMap::load(interior::InteriorId::Office);
    if *location == WorldLocation::Interior {
        spawn_interior_scene(&mut commands, &asset_server, &interior_map, &interior_state);
    }

    // The motel court is only one clearing in a much larger, forageable valley.
    // A tree's small ground footprint must be fully on land; its broad art bounds
    // keep later forage placement from hiding objects beneath the canopy.
    let mut exterior_obstacles = ExteriorObstacles::default();
    for (x, y, size) in TREE_PLACEMENTS {
        let position = resolve_tree_position(&world_grid, Vec2::new(x, y), size);
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
        ));
        exterior_obstacles
            .tree_trunks
            .push(tree_trunk_rect(position, size));
        exterior_obstacles
            .tree_art
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
    for (index, (position, art, size)) in [
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
    ]
    .into_iter()
    .enumerate()
    {
        spawn_safe_world_pickup(
            &mut commands,
            &progression,
            &world_grid,
            &motel,
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
    for (index, position) in [
        Vec2::new(-1_720.0, 930.0),
        Vec2::new(-1_180.0, -920.0),
        Vec2::new(-820.0, 820.0),
        Vec2::new(920.0, -890.0),
        Vec2::new(1_340.0, 1_070.0),
        Vec2::new(1_840.0, -350.0),
    ]
    .into_iter()
    .enumerate()
    {
        spawn_safe_world_pickup(
            &mut commands,
            &progression,
            &world_grid,
            &motel,
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
    for (index, position) in [
        Vec2::new(625.0, -175.0),
        Vec2::new(-1_260.0, 360.0),
        Vec2::new(1_640.0, 520.0),
    ]
    .into_iter()
    .enumerate()
    {
        spawn_safe_world_pickup(
            &mut commands,
            &progression,
            &world_grid,
            &motel,
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
    spawn_safe_world_pickup(
        &mut commands,
        &progression,
        &world_grid,
        &motel,
        &exterior_obstacles,
        &mut pickup_bounds,
        "fallen-ladder-01".to_owned(),
        InteractableKind::Tool,
        Vec2::new(-1_080.0, 1_010.0),
        Vec2::new(44.0, 112.0),
        Sprite::from_image(asset_server.load("world/ladder.png")),
        PickupReward::Tool(ToolId::Ladder),
    );
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
        interior::InteriorId::ALL.len(),
        "the authored motel must have one exterior door per interior"
    );
    door_ids
        .into_iter()
        .zip(interior::InteriorId::ALL)
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
    if !matches!(save.version, 1..=4) || save.vignette_index >= vignettes().len() {
        return;
    }
    story.stage = save.stage;
    story.kindling = save.kindling.min(3);
    story.vignette_index = save.vignette_index;
    story.dialogue_line = save.dialogue_line;
    story.result = save.result;
    story.card = save.card;
    interior_state.0 = save.interior_states;
    motel_access.keys_found = save.motel_keys_found;
    *progression = save.progression;
    story.notice = Some("The old trail returns to memory.".to_owned());
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

fn resolve_tree_position(grid: &terrain::WorldGrid, desired: Vec2, size: f32) -> Vec2 {
    nearest_valid_position(desired, |candidate| {
        let ground = tree_ground_rect(candidate, size);
        grid.supports_land_footprint(ground.center, ground.size)
    })
    .expect("the generated exterior needs enough land for every tree")
}

fn safe_pickup_position(
    grid: &terrain::WorldGrid,
    motel: &interior::MotelExteriorMap,
    obstacles: &ExteriorObstacles,
    reserved: &[ExteriorRect],
    desired: Vec2,
    size: Vec2,
) -> Vec2 {
    nearest_valid_position(desired, |candidate| {
        let bounds = ExteriorRect::new(candidate, size);
        grid.supports_land_footprint(candidate, size)
            && motel.is_area_walkable(candidate, size)
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
    obstacles: &ExteriorObstacles,
    reserved: &mut Vec<ExteriorRect>,
    id: String,
    kind: InteractableKind,
    desired_position: Vec2,
    size: Vec2,
    sprite: Sprite,
    reward: PickupReward,
) -> Entity {
    let position = safe_pickup_position(grid, motel, obstacles, reserved, desired_position, size);
    reserved.push(ExteriorRect::new(position, size));
    spawn_world_pickup(commands, progression, id, kind, position, sprite, reward)
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
    let status_color = Color::srgb(0.92, 0.86, 0.67);
    commands
        .spawn((
            Text::new("📜  "),
            fonts.emoji(18.0),
            TextColor(status_color),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(18.0),
                top: Val::Px(16.0),
                max_width: Val::Px(410.0),
                ..default()
            },
        ))
        .with_child((
            TextSpan::new(""),
            fonts.roman(18.0),
            TextColor(status_color),
            StatusText,
        ));
    commands.spawn((
        Text::new(""),
        fonts.roman(15.0),
        TextColor(Color::srgb(0.86, 0.80, 0.64)),
        TextLayout::new_with_justify(Justify::Right),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(18.0),
            top: Val::Px(16.0),
            max_width: Val::Px(285.0),
            ..default()
        },
        ProgressText,
    ));
    commands
        .spawn((
            Text::new("☞  "),
            fonts.emoji(19.0),
            TextColor(Color::WHITE),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(25.0),
                bottom: Val::Px(18.0),
                width: Val::Percent(50.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_child((
            TextSpan::new(""),
            fonts.roman(19.0),
            TextColor(Color::WHITE),
            PromptText,
        ));

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
}

fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    story: Res<Story>,
    mut environment: MovementEnvironment,
    mut player: Query<(&mut Transform, &mut PlayerAnimation), With<Player>>,
) {
    environment.doorway_attempt.0 = None;
    if matches!(
        story.stage,
        StoryStage::Night
            | StoryStage::Dialogue
            | StoryStage::Interpreting
            | StoryStage::ChoosePaper
            | StoryStage::ChooseIllustration
            | StoryStage::ChooseBorder
            | StoryStage::FinishedCard
            | StoryStage::Epilogue
    ) {
        return;
    }
    let Ok((mut transform, mut animation)) = player.single_mut() else {
        return;
    };
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
        &environment.obstacles,
    ) {
        next.x = next_x.x;
    }
    let next_y = Vec2::new(
        next.x,
        (next.y + delta.y).clamp(-MAP_HALF_HEIGHT, MAP_HALF_HEIGHT),
    );
    let head_probe = next_y + Vec2::Y * DOOR_HEAD_PROBE_OFFSET;
    let doorway = if delta.y > 0.0 && !environment.motel.is_walkable(head_probe) {
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
    obstacles: &ExteriorObstacles,
) -> bool {
    let bounds = player_collision_rect(position);
    grid.supports_land_footprint(bounds.center, bounds.size)
        && motel.is_area_walkable(bounds.center, bounds.size)
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
    player: Query<(&Transform, &Sprite, &Visibility), With<Player>>,
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
        Ok((player_transform, player_sprite, player_visibility)),
        Ok((mut transform, mut sprite, mut visibility)),
    ) = (player.single(), crown.single_mut())
    else {
        return;
    };
    let player_ground = player_transform.translation.truncate() + Vec2::Y * PLAYER_GROUND_OFFSET_Y;
    if *location != WorldLocation::Exterior
        || *player_visibility == Visibility::Hidden
        || !motel.occludes_ground_point(player_ground)
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
    transform.translation.z = building_occlusion_crown_depth(motel.depth_ground_y());
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
    interior_state: Res<InteriorState>,
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
            &interior_state,
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
    mut player: Query<(&Transform, &mut Sprite, &mut PlayerAnimation), With<Player>>,
) {
    let Ok((transform, mut sprite, mut animation)) = player.single_mut() else {
        return;
    };
    let position = transform.translation.truncate();
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

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn handle_interaction(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    nearby: Res<Nearby>,
    mut story: ResMut<Story>,
    interior: Res<interior::InteriorMap>,
    motel: Res<interior::MotelExteriorMap>,
    asset_server: Res<AssetServer>,
    mut resources: InteractionResources,
    mut interactables: Query<&mut Interactable>,
    pickups: Query<&WorldPickup>,
    mut mutable_elements: Query<
        (
            &mut MutableSceneElement,
            &mut Sprite,
            &mut Transform,
            &mut Visibility,
        ),
        Without<Player>,
    >,
) {
    if !keys.just_pressed(KeyCode::KeyE) {
        return;
    }
    let Some(entity) = nearby.0 else {
        return;
    };
    let Ok(mut target) = interactables.get_mut(entity) else {
        return;
    };
    if let Ok(pickup) = pickups.get(entity) {
        match pickup.reward {
            PickupReward::Supply(item, amount) => resources.progression.add_supply(item, amount),
            PickupReward::Tool(tool) => {
                resources.progression.add_tool(tool);
            }
        }
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
            if !resources.progression.spend_supply(SupplyId::Kindling, 3) {
                story.notice = Some(format!(
                    "The hearth needs 3 kindling; you have {}.",
                    resources.progression.supply(SupplyId::Kindling)
                ));
                return;
            }
            if let (Ok((mut instance, mut sprite, mut transform, mut visibility)), Some(element)) = (
                mutable_elements.get_mut(entity),
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
            story.stage = StoryStage::FindBible;
            target.consumed = true;
            story.notice = Some(
                "Flame takes. Warm light reaches into a room untouched for centuries.".to_owned(),
            );
        }
        InteractableKind::Desk => {
            let mut discoveries = Vec::new();
            if !resources.motel_access.keys_found {
                resources.motel_access.keys_found = true;
                resources.progression.add_tool(ToolId::Hammer);
                resources.progression.add_supply(SupplyId::Nails, 12);
                discoveries.push(
                    "A ring of numbered brass keys, a tack hammer, and twelve usable nails wait in the desk's shallow drawer. The other motel doors can now be opened."
                        .to_owned(),
                );
            }
            if story.stage == StoryStage::FindBible {
                story.stage = StoryStage::FindPlank;
                discoveries.push(
                    "Beneath the keys lies a complete book: thin leaves, tiny ordered marks—and you can read them."
                        .to_owned(),
                );
            } else if story.stage == StoryStage::RestoreDesk {
                if let (
                    Ok((mut instance, mut sprite, mut transform, mut visibility)),
                    Some(element),
                ) = (
                    mutable_elements.get_mut(entity),
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
                }
                story.stage = StoryStage::Night;
                story.notice = Some(
                    "The desk stands square again; the first careful carpentry lesson is learned."
                        .to_owned(),
                );
                target.consumed = true;
                return;
            }
            story.notice = Some(if discoveries.is_empty() {
                "The old desk has already yielded its secrets.".to_owned()
            } else {
                discoveries.join("\n\n")
            });
        }
        InteractableKind::Traveler if story.stage == StoryStage::MeetTraveler => {
            story.stage = StoryStage::Dialogue;
            story.dialogue_line = 0;
            story.notice = None;
        }
        InteractableKind::MotelDoor | InteractableKind::InteriorExit => {}
        InteractableKind::InteriorRepairable => {
            let Ok((mut instance, mut sprite, mut transform, mut visibility)) =
                mutable_elements.get_mut(entity)
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
            target.consumed = true;
            story.notice = Some(task_success_notice(element, &outcome));
        }
        InteractableKind::ExteriorRepairable => {
            let Ok((mut instance, mut sprite, mut transform, mut visibility)) =
                mutable_elements.get_mut(entity)
            else {
                return;
            };
            let Some(element) = motel.mutable_element(&instance.id) else {
                return;
            };
            let center = element.states.get("repaired").map_or_else(
                || transform.translation.truncate(),
                |visual| motel.element_center(element, visual.size),
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
    inbox: Res<InterpretInbox>,
) {
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
    if story.stage == StoryStage::Epilogue && keys.just_pressed(KeyCode::KeyR) {
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
    nearby: Res<Nearby>,
    asset_server: Res<AssetServer>,
    interactables: Query<&Interactable>,
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
        StoryStage::FindBible => "Search the room now that you have light.",
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
        **text = format!(
            "RESTORATION\n{}\n\nTOOLS\n{}\n\nSUPPLIES\n{}",
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
            || "WASD / arrows — move     E — interact".to_owned(),
            |(entity, item)| {
                if let Ok(task) = task_targets.get(entity) {
                    return format!(
                        "E — {} this item     [{}]",
                        task.action.infinitive(),
                        task.requirements
                    );
                }
                match item.kind {
                    InteractableKind::Sign => "E — inspect the old sign",
                    InteractableKind::Kindling => "E — gather kindling",
                    InteractableKind::Log => "E — gather a fallen log",
                    InteractableKind::Hearth => "E — tend the hearth",
                    InteractableKind::Plank => "E — take the sound plank",
                    InteractableKind::Tool => "E — take the old ladder",
                    InteractableKind::Desk => "E — search or repair the old desk",
                    InteractableKind::Traveler => "E — welcome the traveler",
                    InteractableKind::MotelDoor => "Walk through the motel door",
                    InteractableKind::InteriorExit => "Walk onto the exit to step outside",
                    InteractableKind::InteriorRepairable => "E — work on this part of the room",
                    InteractableKind::ExteriorRepairable => "E — work on this part of the motel",
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
            "You brace the desk with old cedar and set the book upon it. Smoke rises through a chimney that has been cold longer than any remembered name.\n\nSPACE — sleep until morning"
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
            "The traveler's words settle beside the old book. You search for the need beneath them…"
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
                "{} reads the marks slowly after you speak them aloud. The card disappears into a weathered coat, close to the heart.\n\nBy evening there are new footprints on the old road. Tomorrow, perhaps, there will be another column of smoke answering yours.\n\nR — begin again with another traveler",
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
    use super::*;

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

        let motel_access = MotelAccess { keys_found: true };
        let mut progression = Progression::default();
        progression.add_tool(ToolId::Hammer);
        progression.add_supply(SupplyId::Plank, 2);
        let save = SaveData::capture(&story, &interior_state, &motel_access, &progression);

        assert_eq!(save.version, 4);
        assert_eq!(save.interior_states["motel-room-01/mirror-01"], "repaired");
        assert!(save.motel_keys_found);
        assert!(save.progression.has_tool(ToolId::Hammer));
        assert_eq!(save.progression.supply(SupplyId::Plank), 2);
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

    #[test]
    fn every_tree_resolves_to_a_land_footprint_and_blocks_its_trunk() {
        let grid = terrain::WorldGrid::generate(terrain::WORLD_SEED);
        for (x, y, size) in TREE_PLACEMENTS {
            let position = resolve_tree_position(&grid, Vec2::new(x, y), size);
            let ground = tree_ground_rect(position, size);
            assert!(grid.supports_land_footprint(ground.center, ground.size));

            let obstacles = ExteriorObstacles {
                tree_trunks: vec![tree_trunk_rect(position, size)],
                tree_art: vec![],
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
    fn pickup_safety_moves_the_motel_wall_log_to_clear_land() {
        let grid = terrain::WorldGrid::generate(terrain::WORLD_SEED);
        let motel = interior::MotelExteriorMap::load();
        let obstacles = ExteriorObstacles::default();
        let desired = Vec2::new(-390.0, -80.0);
        let size = Vec2::new(48.0, 34.0);

        assert!(!motel.is_area_walkable(desired, size));
        let actual = safe_pickup_position(&grid, &motel, &obstacles, &[], desired, size);
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
        let occupied = ExteriorRect::new(desired, Vec2::splat(100.0));
        let obstacles = ExteriorObstacles {
            tree_trunks: vec![],
            tree_art: vec![occupied],
        };

        let actual = safe_pickup_position(&grid, &motel, &obstacles, &[], desired, size);
        assert!(!occupied.overlaps(ExteriorRect::new(actual, size)));

        let reserved = [ExteriorRect::new(actual, size)];
        let second = safe_pickup_position(&grid, &motel, &obstacles, &reserved, actual, size);
        assert!(!reserved[0].overlaps(ExteriorRect::new(second, size)));
    }
}
