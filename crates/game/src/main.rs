//! The Waystation at the Edge of the Ash — hackathon vertical slice.

#![allow(clippy::needless_pass_by_value)]

mod interior;
mod terrain;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use terrain::{MAP_HALF_HEIGHT, MAP_HALF_WIDTH};
use waystation_shared::{
    fixture_response, vignettes, CardRecipe, InterpretRequest, InterpretResponse,
};

const PLAYER_SPEED: f32 = 210.0;
const INTERACT_DISTANCE: f32 = 72.0;
const DEVELOPMENT_PRESENTATION_SCALE: f32 = 2.0;
const INTERIOR_CAMERA_SCALE: f32 = 0.72;
const CAMERA_HALF_WIDTH: f32 = 480.0 / DEVELOPMENT_PRESENTATION_SCALE;
const CAMERA_HALF_HEIGHT: f32 = 270.0 / DEVELOPMENT_PRESENTATION_SCALE;
const ROMAN_FONT_PATH: &str = "fonts/EBGaramond-Variable.ttf";
const EMOJI_FONT_PATH: &str = "fonts/NotoEmoji-Variable.ttf";
const MOTEL_DOOR_POSITION: Vec2 = Vec2::new(120.0, -68.0);
const EXTERIOR_DOORSTEP_POSITION: Vec2 = Vec2::new(120.0, -112.0);

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.08, 0.09, 0.08)))
        .insert_resource(UiScale(DEVELOPMENT_PRESENTATION_SCALE))
        .insert_resource(Story::default())
        .insert_resource(InterpretInbox::default())
        .insert_resource(initial_world_location())
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
        .add_systems(
            Startup,
            (load_story, setup_world, load_ui_fonts, setup_ui).chain(),
        )
        .add_systems(
            Update,
            (
                move_player,
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

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct Traveler;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum InteractableKind {
    Sign,
    Kindling,
    Hearth,
    Bible,
    Plank,
    Desk,
    Traveler,
    MotelDoor,
    InteriorExit,
    InteriorRepairable,
}

#[derive(Component)]
struct Interactable {
    kind: InteractableKind,
    consumed: bool,
}

#[derive(Component)]
struct MutableInteriorElement {
    id: String,
    state: String,
}

#[derive(Resource, Default)]
struct InteriorState(HashMap<String, String>);

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
}

impl SaveData {
    #[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
    fn capture(story: &Story, interior_state: &InteriorState) -> Self {
        Self {
            version: 2,
            stage: story.stage,
            kindling: story.kindling,
            vignette_index: story.vignette_index,
            dialogue_line: story.dialogue_line,
            result: story.result.clone(),
            card: story.card.clone(),
            interior_states: interior_state.0.clone(),
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
    commands.insert_resource(world_grid);
    let stone = asset_server.load("world/stone.png");
    let floor = asset_server.load("world/floor.png");
    let tree = asset_server.load("world/tree.png");
    let scribe = asset_server.load("world/scribe.png");
    let interior_map = interior::InteriorMap::motel_room();
    interior::spawn(&mut commands, &asset_server, &interior_map);
    for element in interior_map.mutable_elements() {
        let state_key = format!("{}/{}", interior_map.id, element.id);
        let state = interior_state
            .0
            .get(&state_key)
            .map_or(element.initial_state.as_str(), String::as_str);
        let entity =
            interior::spawn_mutable(&mut commands, &asset_server, &interior_map, element, state);
        commands.entity(entity).insert((
            Interactable {
                kind: InteractableKind::InteriorRepairable,
                consumed: state == "repaired",
            },
            MutableInteriorElement {
                id: element.id.clone(),
                state: state.to_owned(),
            },
        ));
    }

    // Protected valley: tree-shadow slopes and the stone motel court sit over
    // the generated grass, dirt, old road, ponds, and river terrain.
    for (x, y, size) in [
        (-710.0, 420.0, 160.0),
        (-300.0, 430.0, 180.0),
        (100.0, 455.0, 150.0),
        (520.0, 420.0, 200.0),
        (735.0, 190.0, 150.0),
        (700.0, -50.0, 180.0),
        (-725.0, -120.0, 150.0),
    ] {
        commands.spawn((
            Sprite {
                image: tree.clone(),
                custom_size: Some(Vec2::splat(size)),
                ..default()
            },
            Transform::from_xyz(x, y, -6.0),
        ));
    }

    // Motel shell and three rooms.
    spawn_tile_grid(&mut commands, stone, Vec2::new(-144.0, -16.0), 25, 11, -4.0);
    spawn_tile_grid(&mut commands, floor, Vec2::new(-112.0, 16.0), 23, 8, -3.0);
    for x in [-5.0, 245.0, 495.0] {
        spawn_rect(
            &mut commands,
            Vec2::new(x, 105.0),
            Vec2::new(12.0, 230.0),
            Color::srgb(0.36, 0.37, 0.32),
            -2.0,
        );
    }
    spawn_rect(
        &mut commands,
        Vec2::new(245.0, -35.0),
        Vec2::new(780.0, 52.0),
        Color::srgb(0.23, 0.24, 0.20),
        -2.0,
    );

    spawn_interactable(
        &mut commands,
        InteractableKind::Sign,
        Vec2::new(-160.0, -245.0),
        Vec2::new(72.0, 96.0),
        Color::srgb(0.37, 0.24, 0.14),
    );
    // Fallen wood drying under the old growth, from sound logs to loose tinder.
    for (position, art) in [
        (Vec2::new(-390.0, -80.0), "world/kindling_logs.png"),
        (Vec2::new(-285.0, 170.0), "world/kindling_branches.png"),
        (Vec2::new(-80.0, 355.0), "world/kindling_tinder.png"),
    ] {
        spawn_interactable_sprite(
            &mut commands,
            InteractableKind::Kindling,
            position,
            Sprite::from_image(asset_server.load(art)),
        );
    }
    spawn_interactable(
        &mut commands,
        InteractableKind::Hearth,
        Vec2::new(390.0, 135.0),
        Vec2::new(70.0, 62.0),
        Color::srgb(0.16, 0.13, 0.12),
    );
    spawn_interactable(
        &mut commands,
        InteractableKind::Bible,
        Vec2::new(120.0, 130.0),
        Vec2::new(32.0, 24.0),
        Color::srgb(0.36, 0.12, 0.08),
    );
    spawn_interactable(
        &mut commands,
        InteractableKind::Plank,
        Vec2::new(625.0, -175.0),
        Vec2::new(88.0, 24.0),
        Color::srgb(0.48, 0.31, 0.16),
    );
    spawn_interactable(
        &mut commands,
        InteractableKind::Desk,
        Vec2::new(265.0, 125.0),
        Vec2::new(82.0, 54.0),
        Color::srgb(0.30, 0.20, 0.12),
    );
    spawn_interactable(
        &mut commands,
        InteractableKind::MotelDoor,
        MOTEL_DOOR_POSITION,
        Vec2::new(34.0, 22.0),
        Color::srgb(0.24, 0.13, 0.08),
    );
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
    commands.spawn((
        Sprite::from_image(scribe),
        Transform::from_xyz(player_position.x, player_position.y, 5.0),
        Player,
    ));
    spawn_interactable(
        &mut commands,
        InteractableKind::InteriorExit,
        interior_map.cell_center(interior_map.exits[0]),
        Vec2::splat(30.0),
        Color::srgba(0.0, 0.0, 0.0, 0.0),
    );
    commands.insert_resource(interior_map);
    commands.insert_resource(Nearby::default());
}

fn spawn_tile_grid(
    commands: &mut Commands,
    texture: Handle<Image>,
    bottom_left: Vec2,
    columns: u16,
    rows: u16,
    z: f32,
) {
    for row in 0..rows {
        for column in 0..columns {
            commands.spawn((
                Sprite::from_image(texture.clone()),
                Transform::from_xyz(
                    f32::from(column).mul_add(32.0, bottom_left.x),
                    f32::from(row).mul_add(32.0, bottom_left.y),
                    z,
                ),
            ));
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn load_story(mut story: ResMut<Story>, mut interior_state: ResMut<InteriorState>) {
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
    if !matches!(save.version, 1 | 2) || save.vignette_index >= vignettes().len() {
        return;
    }
    story.stage = save.stage;
    story.kindling = save.kindling.min(3);
    story.vignette_index = save.vignette_index;
    story.dialogue_line = save.dialogue_line;
    story.result = save.result;
    story.card = save.card;
    interior_state.0 = save.interior_states;
    story.notice = Some("The old trail returns to memory.".to_owned());
}

#[cfg(not(target_arch = "wasm32"))]
const fn load_story() {}

#[cfg(target_arch = "wasm32")]
fn save_story(story: Res<Story>, interior_state: Res<InteriorState>) {
    if !story.is_changed() && !interior_state.is_changed() {
        return;
    }
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return;
    };
    if let Ok(raw) = serde_json::to_string(&SaveData::capture(&story, &interior_state)) {
        let _ = storage.set_item("waystation-save-v1", &raw);
    }
}

#[cfg(not(target_arch = "wasm32"))]
const fn save_story() {}

fn spawn_rect(commands: &mut Commands, position: Vec2, size: Vec2, color: Color, z: f32) {
    commands.spawn((
        Sprite::from_color(color, size),
        Transform::from_xyz(position.x, position.y, z),
    ));
}

fn spawn_interactable(
    commands: &mut Commands,
    kind: InteractableKind,
    position: Vec2,
    size: Vec2,
    color: Color,
) -> Entity {
    spawn_interactable_sprite(commands, kind, position, Sprite::from_color(color, size))
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

fn load_ui_fonts(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(UiFonts {
        roman: asset_server.load(ROMAN_FONT_PATH),
        emoji: asset_server.load(EMOJI_FONT_PATH),
    });
}

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
    location: Res<WorldLocation>,
    interior: Res<interior::InteriorMap>,
    mut player: Query<&mut Transform, (With<Player>, Without<MutableInteriorElement>)>,
) {
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
    let Ok(mut transform) = player.single_mut() else {
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
        let delta = direction.normalize() * PLAYER_SPEED * time.delta_secs();
        if *location == WorldLocation::Exterior {
            transform.translation.x =
                (transform.translation.x + delta.x).clamp(-MAP_HALF_WIDTH, MAP_HALF_WIDTH);
            transform.translation.y =
                (transform.translation.y + delta.y).clamp(-MAP_HALF_HEIGHT, MAP_HALF_HEIGHT);
        } else {
            let mut next = transform.translation.truncate();
            let next_x = Vec2::new(next.x + delta.x, next.y);
            if interior.is_walkable(next_x) {
                next.x = next_x.x;
            }
            let next_y = Vec2::new(next.x, next.y + delta.y);
            if interior.is_walkable(next_y) {
                next.y = next_y.y;
            }
            transform.translation.x = next.x;
            transform.translation.y = next.y;
        }
    }
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
    keys: Res<ButtonInput<KeyCode>>,
    nearby: Res<Nearby>,
    mut story: ResMut<Story>,
    mut location: ResMut<WorldLocation>,
    interior: Res<interior::InteriorMap>,
    asset_server: Res<AssetServer>,
    mut interior_state: ResMut<InteriorState>,
    mut player: Query<&mut Transform, With<Player>>,
    mut interactables: Query<&mut Interactable>,
    mut mutable_elements: Query<
        (
            &mut MutableInteriorElement,
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
        InteractableKind::Kindling
            if matches!(
                story.stage,
                StoryStage::Arrival | StoryStage::GatherKindling
            ) =>
        {
            target.consumed = true;
            story.kindling += 1;
            story.stage = if story.kindling >= 3 {
                StoryStage::LightHearth
            } else {
                StoryStage::GatherKindling
            };
            story.notice = Some(format!(
                "Dry wood, sheltered beneath the old growth. Kindling: {}/3.",
                story.kindling
            ));
        }
        InteractableKind::Hearth if story.stage == StoryStage::LightHearth => {
            story.stage = StoryStage::FindBible;
            story.notice = Some(
                "Flame takes. Warm light reaches into a room untouched for centuries.".to_owned(),
            );
        }
        InteractableKind::Bible if story.stage == StoryStage::FindBible => {
            story.stage = StoryStage::FindPlank;
            story.notice = Some(
                "A complete book. Thin leaves, tiny ordered marks—and you can read them. Find what the broken desk needs."
                    .to_owned(),
            );
        }
        InteractableKind::Plank if story.stage == StoryStage::FindPlank => {
            target.consumed = true;
            story.stage = StoryStage::RestoreDesk;
            story.notice = Some("Old cedar, still sound beneath the fallen awning.".to_owned());
        }
        InteractableKind::Desk if story.stage == StoryStage::RestoreDesk => {
            story.stage = StoryStage::Night;
            story.notice = None;
        }
        InteractableKind::Traveler if story.stage == StoryStage::MeetTraveler => {
            story.stage = StoryStage::Dialogue;
            story.dialogue_line = 0;
            story.notice = None;
        }
        InteractableKind::MotelDoor => {
            if let Ok(mut transform) = player.single_mut() {
                let position = interior.cell_center(interior.entry);
                transform.translation.x = position.x;
                transform.translation.y = position.y;
                *location = WorldLocation::Interior;
                story.notice = Some(format!(
                    "Inside {}, the valley light falls away behind you.",
                    interior.name
                ));
            }
        }
        InteractableKind::InteriorExit => {
            if let Ok(mut transform) = player.single_mut() {
                transform.translation.x = EXTERIOR_DOORSTEP_POSITION.x;
                transform.translation.y = EXTERIOR_DOORSTEP_POSITION.y;
                *location = WorldLocation::Exterior;
                story.notice = Some("You step back into the valley air.".to_owned());
            }
        }
        InteractableKind::InteriorRepairable => {
            let Ok((mut instance, mut sprite, mut transform, mut visibility)) =
                mutable_elements.get_mut(entity)
            else {
                return;
            };
            let Some(element) = interior.mutable_element(&instance.id) else {
                return;
            };
            let Some(repaired) = element.states.get("repaired") else {
                story.notice = Some(format!("{} cannot be repaired yet.", element.label));
                return;
            };
            if let Some(path) = &repaired.image_path {
                sprite.image = asset_server.load(path.clone());
            }
            sprite.custom_size = Some(repaired.size.max(Vec2::ONE));
            let center = interior.element_center(element, repaired.size);
            transform.translation.x = center.x;
            transform.translation.y = center.y;
            *visibility = if repaired.visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
            "repaired".clone_into(&mut instance.state);
            interior_state.0.insert(
                format!("{}/{}", interior.id, instance.id),
                instance.state.clone(),
            );
            target.consumed = true;
            story.notice = Some(format!(
                "You restore the {}. The {} is sound again.",
                element.label, element.kind
            ));
        }
        _ => {
            story.notice = Some("There may be a use for this later.".to_owned());
        }
    }
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
    mut sprites: Query<(&mut Interactable, &mut Sprite), Without<Traveler>>,
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
    for (mut interactable, mut sprite) in &mut sprites {
        if interactable.kind == InteractableKind::Kindling {
            if story.stage == StoryStage::Arrival {
                interactable.consumed = false;
            }
            // Gathered wood leaves the ground; a replay puts every pile back.
            sprite.color = if interactable.consumed {
                Color::srgba(0.0, 0.0, 0.0, 0.0)
            } else {
                Color::WHITE
            };
        }
        if interactable.kind == InteractableKind::Plank {
            interactable.consumed = !matches!(
                story.stage,
                StoryStage::Arrival
                    | StoryStage::GatherKindling
                    | StoryStage::LightHearth
                    | StoryStage::FindBible
                    | StoryStage::FindPlank
            );
            sprite.color = if interactable.consumed {
                Color::srgba(0.0, 0.0, 0.0, 0.0)
            } else {
                Color::srgb(0.48, 0.31, 0.16)
            };
        }
        if interactable.kind == InteractableKind::Hearth && story.stage != StoryStage::LightHearth {
            sprite.color = if matches!(
                story.stage,
                StoryStage::FindBible
                    | StoryStage::FindPlank
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
            ) {
                Color::srgb(0.94, 0.39, 0.10)
            } else {
                Color::srgb(0.16, 0.13, 0.12)
            };
        }
    }
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_ui(
    story: Res<Story>,
    nearby: Res<Nearby>,
    asset_server: Res<AssetServer>,
    interactables: Query<&Interactable>,
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
        ),
    >,
    mut card_art: Query<(&mut ImageNode, &mut Visibility), (With<CardArt>, Without<OverlayRoot>)>,
) {
    let objective = match story.stage {
        StoryStage::Arrival => "Explore the standing stones. Find out what this place was.",
        StoryStage::GatherKindling => "Gather dry kindling for the motel hearth.",
        StoryStage::LightHearth => "Bring the kindling to the hearth in the eastern room.",
        StoryStage::FindBible => "Search the room now that you have light.",
        StoryStage::FindPlank => "Find sound wood beneath the motel awning.",
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

    let nearby_prompt = nearby
        .0
        .and_then(|entity| interactables.get(entity).ok())
        .map_or(
            "WASD / arrows — move     E — interact",
            |item| match item.kind {
                InteractableKind::Sign => "E — inspect the old sign",
                InteractableKind::Kindling => "E — gather kindling",
                InteractableKind::Hearth => "E — tend the hearth",
                InteractableKind::Bible => "E — open the nightstand",
                InteractableKind::Plank => "E — take the cedar plank",
                InteractableKind::Desk => "E — repair the writing desk",
                InteractableKind::Traveler => "E — welcome the traveler",
                InteractableKind::MotelDoor => "E — enter the motel room",
                InteractableKind::InteriorExit => "E — step back outside",
                InteractableKind::InteriorRepairable => "E — repair this part of the room",
            },
        );
    if let Ok(mut text) = prompt.single_mut() {
        nearby_prompt.clone_into(&mut *text);
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

        let save = SaveData::capture(&story, &interior_state);

        assert_eq!(save.version, 2);
        assert_eq!(save.interior_states["motel-room-01/mirror-01"], "repaired");
    }
}
