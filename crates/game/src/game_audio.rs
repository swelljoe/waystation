//! Ambient music, weather mixing, and authored-surface sound effects.

use std::time::Duration;

use bevy::{
    audio::{AudioSinkPlayback, Volume},
    prelude::*,
};

use crate::{interior::MutableElement, MutableSceneElement, Player, WorldLocation};

const MUSIC_PATHS: [&str; 2] = [
    "audio/music/andriig-sad-instrumental.mp3",
    "audio/music/andriig-sad-acoustic.mp3",
];
const RAIN_PATH: &str = "audio/ambience/relaxing-rain.mp3";
const INDOOR_RAIN_PATH: &str = "audio/ambience/relaxing-rain-indoors.mp3";
const FLOORBOARD_CREAK_PATHS: [&str; 3] = [
    "audio/sfx/floorboard-creak-01.mp3",
    "audio/sfx/floorboard-creak-02.mp3",
    "audio/sfx/floorboard-creak-03.mp3",
];
const HAMMERING_PATH: &str = "audio/sfx/hammering.mp3";

const MUSIC_VOLUME: f32 = 0.10;
const MUSIC_GAP_SECONDS: f32 = 12.0;
const RAIN_EXTERIOR_VOLUME: f32 = 0.22;
const RAIN_INTERIOR_VOLUME: f32 = 0.05;
const RAIN_FADE_PER_SECOND: f32 = 0.14;
const GUARANTEED_RAIN_SECONDS: f32 = 6.0 * 60.0;
const WEATHER_PHASE_SECONDS: [f32; 6] = [85.0, 180.0, 110.0, 240.0, 70.0, 150.0];
const FLOORBOARD_CREAK_VOLUME: f32 = 0.24;
const FLOORBOARD_CREAK_INTERVAL: f32 = 0.72;
const HAMMERING_VOLUME: f32 = 0.34;

pub struct GameAudioPlugin;

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_audio).add_systems(
            Update,
            (manage_music, mix_rain, play_floorboard_creaks)
                .after(crate::handle_automatic_doorways),
        );
    }
}

#[derive(Component)]
struct RainLoop {
    indoors: bool,
}

#[derive(Component)]
pub struct CreakingFloorboard;

#[derive(Resource)]
struct AudioLibrary {
    music: [Handle<AudioSource>; 2],
    creaks: [Handle<AudioSource>; 3],
}

#[derive(Resource)]
struct MusicState {
    current: Option<Entity>,
    next_track: usize,
    gap: Timer,
}

#[derive(Resource)]
struct WeatherState {
    guaranteed_rain: Timer,
    phase: Timer,
    phase_index: usize,
    raining: bool,
}

impl Default for WeatherState {
    fn default() -> Self {
        Self {
            guaranteed_rain: Timer::from_seconds(GUARANTEED_RAIN_SECONDS, TimerMode::Once),
            phase: Timer::from_seconds(WEATHER_PHASE_SECONDS[0], TimerMode::Once),
            phase_index: 0,
            raining: true,
        }
    }
}

#[derive(Resource, Default)]
struct FloorboardAudioState {
    previous_player_position: Option<Vec2>,
    active_board: Option<Entity>,
    cooldown_seconds: f32,
    next_clip: usize,
}

pub fn is_creaking_floorboard(element: &MutableElement) -> bool {
    element.kind == "floor" && element.label.starts_with("Broken Floorboards")
}

fn setup_audio(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    location: Res<WorldLocation>,
) {
    let music = MUSIC_PATHS.map(|path| asset_server.load(path));
    let creaks = FLOORBOARD_CREAK_PATHS.map(|path| asset_server.load(path));
    let current = spawn_music(&mut commands, music[0].clone());
    let rain_volume = rain_target_volume(*location, true, false);
    commands.spawn((
        AudioPlayer::new(asset_server.load(RAIN_PATH)),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(rain_volume)),
        RainLoop { indoors: false },
    ));
    let indoor_rain_volume = rain_target_volume(*location, true, true);
    commands.spawn((
        AudioPlayer::new(asset_server.load(INDOOR_RAIN_PATH)),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(indoor_rain_volume)),
        RainLoop { indoors: true },
    ));
    commands.insert_resource(AudioLibrary { music, creaks });
    commands.insert_resource(MusicState {
        current: Some(current),
        next_track: 1,
        gap: Timer::from_seconds(MUSIC_GAP_SECONDS, TimerMode::Once),
    });
    commands.insert_resource(WeatherState::default());
    commands.insert_resource(FloorboardAudioState::default());
}

fn spawn_music(commands: &mut Commands, source: Handle<AudioSource>) -> Entity {
    commands
        .spawn((
            AudioPlayer::new(source),
            PlaybackSettings::ONCE.with_volume(Volume::Linear(MUSIC_VOLUME)),
        ))
        .id()
}

pub fn play_hammering(commands: &mut Commands, asset_server: &AssetServer) {
    commands.spawn((
        AudioPlayer::new(asset_server.load(HAMMERING_PATH)),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(HAMMERING_VOLUME)),
    ));
}

fn manage_music(
    mut commands: Commands,
    time: Res<Time>,
    library: Res<AudioLibrary>,
    mut state: ResMut<MusicState>,
    sinks: Query<&AudioSink>,
) {
    if let Some(current) = state.current {
        if sinks.get(current).is_ok_and(AudioSinkPlayback::empty) {
            commands.entity(current).despawn();
            state.current = None;
            state.gap.reset();
        }
        return;
    }

    state.gap.tick(time.delta());
    if state.gap.just_finished() {
        let track = library.music[state.next_track].clone();
        state.next_track = (state.next_track + 1) % library.music.len();
        state.current = Some(spawn_music(&mut commands, track));
    }
}

fn mix_rain(
    time: Res<Time>,
    location: Res<WorldLocation>,
    mut weather: ResMut<WeatherState>,
    mut rain: Query<(&RainLoop, &mut AudioSink)>,
) {
    advance_weather(&mut weather, time.delta());
    let max_change = RAIN_FADE_PER_SECOND * time.delta_secs();
    for (layer, mut sink) in &mut rain {
        let target = rain_target_volume(*location, weather.raining, layer.indoors);
        let volume = approach(sink.volume().to_linear(), target, max_change);
        sink.set_volume(Volume::Linear(volume));
    }
}

fn advance_weather(weather: &mut WeatherState, delta: Duration) {
    if !weather.guaranteed_rain.is_finished() {
        weather.guaranteed_rain.tick(delta);
        if weather.guaranteed_rain.just_finished() {
            weather.raining = false;
            weather.phase.reset();
        }
        return;
    }

    weather.phase.tick(delta);
    if weather.phase.just_finished() {
        weather.raining = !weather.raining;
        weather.phase_index = (weather.phase_index + 1) % WEATHER_PHASE_SECONDS.len();
        weather.phase.set_duration(Duration::from_secs_f32(
            WEATHER_PHASE_SECONDS[weather.phase_index],
        ));
        weather.phase.reset();
    }
}

const fn rain_target_volume(location: WorldLocation, raining: bool, indoors_track: bool) -> f32 {
    if !raining {
        0.0
    } else if matches!(location, WorldLocation::Interior) && indoors_track {
        RAIN_INTERIOR_VOLUME
    } else if matches!(location, WorldLocation::Exterior) && !indoors_track {
        RAIN_EXTERIOR_VOLUME
    } else {
        0.0
    }
}

fn approach(current: f32, target: f32, max_change: f32) -> f32 {
    if current < target {
        (current + max_change).min(target)
    } else {
        (current - max_change).max(target)
    }
}

#[allow(clippy::too_many_arguments)]
fn play_floorboard_creaks(
    mut commands: Commands,
    time: Res<Time>,
    location: Res<WorldLocation>,
    library: Res<AudioLibrary>,
    player: Query<&Transform, With<Player>>,
    boards: Query<
        (
            Entity,
            &Transform,
            &Sprite,
            &MutableSceneElement,
            &Visibility,
        ),
        With<CreakingFloorboard>,
    >,
    mut state: ResMut<FloorboardAudioState>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    let player_position = player.translation.truncate();
    let moved = state
        .previous_player_position
        .is_none_or(|previous| previous.distance_squared(player_position) > 0.25);
    state.previous_player_position = Some(player_position);
    state.cooldown_seconds = (state.cooldown_seconds - time.delta_secs()).max(0.0);

    if *location != WorldLocation::Interior {
        state.active_board = None;
        return;
    }

    let active_board = boards
        .iter()
        .find(|(_, transform, sprite, mutable, visibility)| {
            mutable.state == "damaged"
                && **visibility == Visibility::Visible
                && point_in_sprite(
                    player_position,
                    transform.translation.truncate(),
                    sprite.custom_size.unwrap_or(Vec2::ONE),
                )
        })
        .map(|(entity, ..)| entity);
    let entered_board = active_board.is_some() && active_board != state.active_board;
    state.active_board = active_board;

    if active_board.is_some() && moved && (entered_board || state.cooldown_seconds == 0.0) {
        commands.spawn((
            AudioPlayer::new(library.creaks[state.next_clip].clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(FLOORBOARD_CREAK_VOLUME)),
        ));
        state.next_clip = (state.next_clip + 1) % library.creaks.len();
        state.cooldown_seconds = FLOORBOARD_CREAK_INTERVAL;
    }
}

fn point_in_sprite(point: Vec2, center: Vec2, size: Vec2) -> bool {
    let half_size = size / 2.0;
    (point.x - center.x).abs() <= half_size.x && (point.y - center.y).abs() <= half_size.y
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < f32::EPSILON);
    }

    #[test]
    fn indoor_rain_is_present_but_quiet() {
        let outside = rain_target_volume(WorldLocation::Exterior, true, false);
        let inside = rain_target_volume(WorldLocation::Interior, true, true);
        assert!(inside > 0.0);
        assert!(inside < outside / 4.0);
        assert_close(
            rain_target_volume(WorldLocation::Exterior, false, false),
            0.0,
        );
        assert_close(
            rain_target_volume(WorldLocation::Interior, true, false),
            0.0,
        );
        assert_close(rain_target_volume(WorldLocation::Exterior, true, true), 0.0);
    }

    #[test]
    fn rain_mix_approaches_target_without_overshooting() {
        assert_close(approach(0.18, 0.014, 0.1), 0.08);
        assert_close(approach(0.02, 0.014, 0.1), 0.014);
        assert_close(approach(0.0, 0.18, 0.25), 0.18);
    }

    #[test]
    fn floorboard_hit_area_uses_authored_sprite_dimensions() {
        assert!(point_in_sprite(
            Vec2::new(20.0, 10.0),
            Vec2::ZERO,
            Vec2::new(48.0, 24.0)
        ));
        assert!(!point_in_sprite(
            Vec2::new(25.0, 10.0),
            Vec2::ZERO,
            Vec2::new(48.0, 24.0)
        ));
    }

    #[test]
    fn initial_rain_eventually_alternates_with_dry_weather() {
        let mut weather = WeatherState::default();
        advance_weather(
            &mut weather,
            Duration::from_secs_f32(GUARANTEED_RAIN_SECONDS),
        );
        assert!(!weather.raining);
        advance_weather(
            &mut weather,
            Duration::from_secs_f32(WEATHER_PHASE_SECONDS[0]),
        );
        assert!(weather.raining);
    }
}
