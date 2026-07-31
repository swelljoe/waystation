//! Deterministic world generation and dual-grid terrain rendering.
//!
//! The logical `WorldGrid` is the source of truth. Grass and dirt are rendered
//! at the intersections between four logical cells, using a four-corner mask.
//! Water remains a cell-centered overlay with eight-neighbor transitions.

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]

use bevy::{prelude::*, sprite::Text2dShadow};

pub const TILE_SIZE: f32 = 32.0;
pub const MAP_WIDTH: usize = 144;
pub const MAP_HEIGHT: usize = 96;
pub const MAP_HALF_WIDTH: f32 = MAP_WIDTH as f32 * TILE_SIZE / 2.0;
pub const MAP_HALF_HEIGHT: f32 = MAP_HEIGHT as f32 * TILE_SIZE / 2.0;
pub const WORLD_SEED: u64 = 0x5741_5953_5441_5449;

const ATLAS_COLUMNS: u32 = 35;

const GRASS_PLAIN: usize = 0;
const GRASS_PLAIN_ALT: usize = 1;
const GRASS_FLOWERS: usize = 2;
const GRASS_ROCKS: usize = 3;

const DIRT_FULL_A: usize = 4;
const DIRT_FULL_B: usize = 5;
const DIRT_NORTH: usize = 10;
const DIRT_EAST: usize = 11;
const DIRT_SOUTH: usize = 12;
const DIRT_WEST: usize = 13;
const DIRT_EXCEPT_NW: usize = 14;
const DIRT_EXCEPT_NE: usize = 15;
const DIRT_EXCEPT_SE: usize = 16;
const DIRT_EXCEPT_SW: usize = 17;

const WATER_CENTER: usize = 18;
const WATER_ISOLATED: usize = 19;
const WATER_ISOLATED_SMALL: usize = 20;
const WATER_CAP_W: usize = 21;
const WATER_CAP_E: usize = 22;
const WATER_OUTER_NW: usize = 23;
const WATER_OUTER_NE: usize = 24;
const WATER_OUTER_SW: usize = 25;
const WATER_OUTER_SE: usize = 26;
const WATER_EDGE_N: usize = 27;
const WATER_EDGE_E: usize = 28;
const WATER_EDGE_S: usize = 29;
const WATER_EDGE_W: usize = 30;
const WATER_INNER_NW: usize = 31;
const WATER_INNER_NE: usize = 32;
const WATER_INNER_SE: usize = 33;
const WATER_INNER_SW: usize = 34;

const DEBUG_HALF_COLUMNS: usize = 9;
const DEBUG_HALF_ROWS: usize = 6;

#[derive(Clone, Copy)]
struct TileDebugInfo {
    role: &'static str,
    source_column: u8,
    source_row: u8,
}

const TILE_DEBUG_INFO: [TileDebugInfo; ATLAS_COLUMNS as usize] = [
    TileDebugInfo::new("G.plain", 0, 0),
    TileDebugInfo::new("G.plain_alt", 0, 0),
    TileDebugInfo::new("G.flowers", 0, 1),
    TileDebugInfo::new("G.rocks", 1, 0),
    TileDebugInfo::new("DG.full_a", 0, 2),
    TileDebugInfo::new("DG.full_b", 0, 2),
    TileDebugInfo::new("DG.only_nw", 4, 0),
    TileDebugInfo::new("DG.only_ne", 5, 0),
    TileDebugInfo::new("DG.only_se", 5, 1),
    TileDebugInfo::new("DG.only_sw", 4, 1),
    TileDebugInfo::new("DG.north", 6, 2),
    TileDebugInfo::new("DG.east", 7, 2),
    TileDebugInfo::new("DG.south", 6, 3),
    TileDebugInfo::new("DG.west", 7, 3),
    TileDebugInfo::new("DG.except_nw", 5, 3),
    TileDebugInfo::new("DG.except_ne", 4, 3),
    TileDebugInfo::new("DG.except_se", 4, 2),
    TileDebugInfo::new("DG.except_sw", 5, 2),
    TileDebugInfo::new("W.center", 40, 0),
    TileDebugInfo::new("W.isolated", 32, 1),
    TileDebugInfo::new("W.isolated_small", 33, 1),
    TileDebugInfo::new("W.cap_w", 32, 0),
    TileDebugInfo::new("W.cap_e", 33, 0),
    TileDebugInfo::new("W.outer_nw", 34, 0),
    TileDebugInfo::new("W.outer_ne", 35, 0),
    TileDebugInfo::new("W.outer_sw", 34, 1),
    TileDebugInfo::new("W.outer_se", 35, 1),
    TileDebugInfo::new("W.edge_n", 38, 1),
    TileDebugInfo::new("W.edge_e", 36, 0),
    TileDebugInfo::new("W.edge_s", 38, 0),
    TileDebugInfo::new("W.edge_w", 36, 1),
    TileDebugInfo::new("W.inner_nw", 37, 1),
    TileDebugInfo::new("W.inner_ne", 39, 1),
    TileDebugInfo::new("W.inner_se", 37, 0),
    TileDebugInfo::new("W.inner_sw", 39, 0),
];

impl TileDebugInfo {
    const fn new(role: &'static str, source_column: u8, source_row: u8) -> Self {
        Self {
            role,
            source_column,
            source_row,
        }
    }
}

#[derive(Resource, Default)]
pub struct TerrainDebugOverlay {
    enabled: bool,
    rendered_center: Option<(usize, usize)>,
}

#[derive(Component)]
pub struct TerrainDebugLabel;

#[derive(Component)]
pub struct TerrainDebugLegend;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Terrain {
    Grass,
    Dirt,
    Water,
}

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct WorldGrid {
    cells: Vec<Terrain>,
}

impl WorldGrid {
    #[must_use]
    pub fn generate(seed: u64) -> Self {
        let mut grid = Self {
            cells: vec![Terrain::Grass; MAP_WIDTH * MAP_HEIGHT],
        };
        grid.grow_dirt(seed);
        grid.remove_dirt_speckles();
        grid.clear_waystation();
        grid.lay_old_road();
        grid.add_water(seed);
        grid.enforce_grass_shores();
        grid.resolve_dirt_checkerboards(seed);
        grid
    }

    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> Terrain {
        self.cells[y * MAP_WIDTH + x]
    }

    /// Checks every logical cell touched by an object's ground footprint.
    #[must_use]
    pub fn supports_land_footprint(&self, center: Vec2, size: Vec2) -> bool {
        let half = size / 2.0;
        let min = center - half;
        let max = center + half;
        if min.x < -MAP_HALF_WIDTH
            || min.y < -MAP_HALF_HEIGHT
            || max.x >= MAP_HALF_WIDTH
            || max.y >= MAP_HALF_HEIGHT
        {
            return false;
        }
        let min_x = ((min.x + MAP_HALF_WIDTH) / TILE_SIZE).floor() as usize;
        let max_x = ((max.x + MAP_HALF_WIDTH) / TILE_SIZE).floor() as usize;
        let min_y = ((min.y + MAP_HALF_HEIGHT) / TILE_SIZE).floor() as usize;
        let max_y = ((max.y + MAP_HALF_HEIGHT) / TILE_SIZE).floor() as usize;
        (min_y..=max_y).all(|y| (min_x..=max_x).all(|x| self.get(x, y) != Terrain::Water))
    }

    fn get_clamped(&self, x: isize, y: isize) -> Terrain {
        let x = x.clamp(0, MAP_WIDTH as isize - 1) as usize;
        let y = y.clamp(0, MAP_HEIGHT as isize - 1) as usize;
        self.get(x, y)
    }

    fn set(&mut self, x: usize, y: usize, terrain: Terrain) {
        self.cells[y * MAP_WIDTH + x] = terrain;
    }

    fn grow_dirt(&mut self, seed: u64) {
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let broad = value_noise(seed ^ 0xD1A7, x as f32 / 13.0, y as f32 / 13.0);
                let detail = value_noise(seed ^ 0xB04D, x as f32 / 6.0, y as f32 / 6.0);
                if broad.mul_add(0.78, detail * 0.22) > 0.61 {
                    self.set(x, y, Terrain::Dirt);
                }
            }
        }
    }

    fn remove_dirt_speckles(&mut self) {
        for _ in 0..2 {
            let previous = self.clone();
            for y in 0..MAP_HEIGHT {
                for x in 0..MAP_WIDTH {
                    if previous.get(x, y) != Terrain::Dirt {
                        continue;
                    }
                    let neighbors = ORTHOGONAL_OFFSETS
                        .iter()
                        .filter(|&&(dx, dy)| {
                            previous.get_clamped(x as isize + dx, y as isize + dy) == Terrain::Dirt
                        })
                        .count();
                    if neighbors < 2 {
                        self.set(x, y, Terrain::Grass);
                    }
                }
            }
        }
    }

    fn clear_waystation(&mut self) {
        let center = (MAP_WIDTH / 2, MAP_HEIGHT / 2);
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                if x.abs_diff(center.0) <= 16 && y.abs_diff(center.1) <= 11 {
                    self.set(x, y, Terrain::Grass);
                }
            }
        }
    }

    fn lay_old_road(&mut self) {
        let first_row = MAP_HEIGHT / 2 - 11;
        for y in first_row..first_row + 3 {
            for x in MAP_WIDTH / 4..MAP_WIDTH {
                self.set(x, y, Terrain::Dirt);
            }
        }
    }

    fn add_water(&mut self, seed: u64) {
        for y in 0..MAP_HEIGHT {
            let meander = value_noise(seed ^ 0xA73E, y as f32 / 11.0, 4.25);
            let center = 11 + (meander * 7.0).round() as usize;
            let half_width =
                2 + usize::from(cell_hash(seed ^ 0x71AE, 0, y as u64).is_multiple_of(5));
            for x in center.saturating_sub(half_width)..=(center + half_width).min(MAP_WIDTH - 1) {
                self.set(x, y, Terrain::Water);
            }
        }
        self.stamp_pond(MAP_WIDTH * 3 / 4, MAP_HEIGHT * 3 / 4, 5.5);
        self.stamp_pond(MAP_WIDTH / 3, MAP_HEIGHT * 5 / 6, 3.75);
    }

    fn stamp_pond(&mut self, center_x: usize, center_y: usize, radius: f32) {
        let reach = radius.ceil() as isize;
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let x = center_x as isize + dx;
                let y = center_y as isize + dy;
                if x < 0 || y < 0 || x >= MAP_WIDTH as isize || y >= MAP_HEIGHT as isize {
                    continue;
                }
                if (dx as f32).hypot(dy as f32) <= radius {
                    self.set(x as usize, y as usize, Terrain::Water);
                }
            }
        }
    }

    fn enforce_grass_shores(&mut self) {
        let previous = self.clone();
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                if previous.get(x, y) != Terrain::Water {
                    continue;
                }
                for &(dx, dy) in &NEIGHBOR_OFFSETS {
                    let nx = x as isize + dx;
                    let ny = y as isize + dy;
                    if nx < 0 || ny < 0 || nx >= MAP_WIDTH as isize || ny >= MAP_HEIGHT as isize {
                        continue;
                    }
                    let (nx, ny) = (nx as usize, ny as usize);
                    if previous.get(nx, ny) != Terrain::Water {
                        self.set(nx, ny, Terrain::Grass);
                    }
                }
            }
        }
    }

    fn resolve_dirt_checkerboards(&mut self, seed: u64) {
        // The source art deliberately omits the two ambiguous marching-squares
        // masks. Eroding one of the diagonally touching dirt cells preserves a
        // single, readable boundary and can never violate a grass water shore.
        loop {
            let mut changed = false;
            for y in 0..MAP_HEIGHT - 1 {
                for x in 0..MAP_WIDTH - 1 {
                    let mask = DualGridMask::from_cells(
                        self.get(x, y + 1),
                        self.get(x + 1, y + 1),
                        self.get(x + 1, y),
                        self.get(x, y),
                    );
                    if !mask.is_checkerboard() {
                        continue;
                    }
                    let candidates = match mask.0 {
                        DualGridMask::CHECKERBOARD_NW_SE => [(x, y + 1), (x + 1, y)],
                        DualGridMask::CHECKERBOARD_NE_SW => [(x + 1, y + 1), (x, y)],
                        _ => continue,
                    };
                    let remove =
                        candidates[(cell_hash(seed ^ 0xD0A1, x as u64, y as u64) & 1) as usize];
                    self.set(remove.0, remove.1, Terrain::Grass);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    fn neighbors(&self, x: usize, y: usize, terrain: Terrain) -> TileNeighbors {
        let x = x as isize;
        let y = y as isize;
        let same = |dx, dy| self.get_clamped(x + dx, y + dy) == terrain;
        let mut mask = 0;
        for (bit, dx, dy) in [
            (TileNeighbors::N, 0, 1),
            (TileNeighbors::NE, 1, 1),
            (TileNeighbors::E, 1, 0),
            (TileNeighbors::SE, 1, -1),
            (TileNeighbors::S, 0, -1),
            (TileNeighbors::SW, -1, -1),
            (TileNeighbors::W, -1, 0),
            (TileNeighbors::NW, -1, 1),
        ] {
            if same(dx, dy) {
                mask |= bit;
            }
        }
        TileNeighbors(mask)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct DualGridMask(u8);

impl DualGridMask {
    const NW: u8 = 1 << 0;
    const NE: u8 = 1 << 1;
    const SE: u8 = 1 << 2;
    const SW: u8 = 1 << 3;
    const CHECKERBOARD_NW_SE: u8 = Self::NW | Self::SE;
    const CHECKERBOARD_NE_SW: u8 = Self::NE | Self::SW;

    const fn from_cells(nw: Terrain, ne: Terrain, se: Terrain, sw: Terrain) -> Self {
        let mut mask = 0;
        if matches!(nw, Terrain::Dirt) {
            mask |= Self::NW;
        }
        if matches!(ne, Terrain::Dirt) {
            mask |= Self::NE;
        }
        if matches!(se, Terrain::Dirt) {
            mask |= Self::SE;
        }
        if matches!(sw, Terrain::Dirt) {
            mask |= Self::SW;
        }
        Self(mask)
    }

    fn at_intersection(grid: &WorldGrid, x: usize, y: usize) -> Self {
        // Render coordinates are one-based around the zero-based logical grid:
        // W(x,y) is shared by R(x+1,y+1), R(x+2,y+1), R(x+1,y+2), and
        // R(x+2,y+2).
        let x = x as isize - 1;
        let y = y as isize - 1;
        Self::from_cells(
            grid.get_clamped(x - 1, y),
            grid.get_clamped(x, y),
            grid.get_clamped(x, y - 1),
            grid.get_clamped(x - 1, y - 1),
        )
    }

    const fn is_checkerboard(self) -> bool {
        matches!(self.0, Self::CHECKERBOARD_NW_SE | Self::CHECKERBOARD_NE_SW)
    }

    const fn corner(self, bit: u8) -> char {
        if self.0 & bit == 0 {
            'G'
        } else {
            'D'
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct TileNeighbors(u8);

impl TileNeighbors {
    const N: u8 = 1 << 0;
    const NE: u8 = 1 << 1;
    const E: u8 = 1 << 2;
    const SE: u8 = 1 << 3;
    const S: u8 = 1 << 4;
    const SW: u8 = 1 << 5;
    const W: u8 = 1 << 6;
    const NW: u8 = 1 << 7;

    const fn contains(self, bit: u8) -> bool {
        self.0 & bit != 0
    }
}

const ORTHOGONAL_OFFSETS: [(isize, isize); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];
const NEIGHBOR_OFFSETS: [(isize, isize); 8] = [
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
];

#[derive(Clone, Copy)]
struct TransitionTiles {
    center_a: usize,
    center_b: usize,
    outer_nw: usize,
    outer_ne: usize,
    outer_sw: usize,
    outer_se: usize,
    edge_n: usize,
    edge_e: usize,
    edge_s: usize,
    edge_w: usize,
    inner_nw: usize,
    inner_ne: usize,
    inner_se: usize,
    inner_sw: usize,
}

const WATER_TILES: TransitionTiles = TransitionTiles {
    center_a: WATER_CENTER,
    center_b: WATER_CENTER,
    outer_nw: WATER_OUTER_NW,
    outer_ne: WATER_OUTER_NE,
    outer_sw: WATER_OUTER_SW,
    outer_se: WATER_OUTER_SE,
    edge_n: WATER_EDGE_N,
    edge_e: WATER_EDGE_E,
    edge_s: WATER_EDGE_S,
    edge_w: WATER_EDGE_W,
    inner_nw: WATER_INNER_NW,
    inner_ne: WATER_INNER_NE,
    inner_se: WATER_INNER_SE,
    inner_sw: WATER_INNER_SW,
};

const fn transition_index(tiles: TransitionTiles, neighbors: TileNeighbors, variant: u64) -> usize {
    let n = neighbors.contains(TileNeighbors::N);
    let ne = neighbors.contains(TileNeighbors::NE);
    let e = neighbors.contains(TileNeighbors::E);
    let se = neighbors.contains(TileNeighbors::SE);
    let s = neighbors.contains(TileNeighbors::S);
    let sw = neighbors.contains(TileNeighbors::SW);
    let w = neighbors.contains(TileNeighbors::W);
    let nw = neighbors.contains(TileNeighbors::NW);
    if !n && !w {
        tiles.outer_nw
    } else if !n && !e {
        tiles.outer_ne
    } else if !s && !w {
        tiles.outer_sw
    } else if !s && !e {
        tiles.outer_se
    } else if !n {
        tiles.edge_n
    } else if !e {
        tiles.edge_e
    } else if !s {
        tiles.edge_s
    } else if !w {
        tiles.edge_w
    } else if !nw {
        tiles.inner_nw
    } else if !ne {
        tiles.inner_ne
    } else if !se {
        tiles.inner_se
    } else if !sw {
        tiles.inner_sw
    } else if variant & 1 == 0 {
        tiles.center_a
    } else {
        tiles.center_b
    }
}

const fn grass_index(variant: u64) -> usize {
    match variant % 100 {
        0..=84 => GRASS_PLAIN,
        85..=95 => GRASS_PLAIN_ALT,
        96..=98 => GRASS_FLOWERS,
        _ => GRASS_ROCKS,
    }
}

const fn dual_grid_index(mask: DualGridMask, variant: u64) -> usize {
    // THE GROUND draws terrain boundaries hugging cell edges and has no piece
    // for a lone dirt corner, so single-corner masks (and the checkerboards
    // the generator removes) render as grass; convex dirt corners round off.
    match mask.0 {
        3 => DIRT_NORTH,
        6 => DIRT_EAST,
        12 => DIRT_SOUTH,
        9 => DIRT_WEST,
        14 => DIRT_EXCEPT_NW,
        13 => DIRT_EXCEPT_NE,
        11 => DIRT_EXCEPT_SE,
        7 => DIRT_EXCEPT_SW,
        15 if variant & 1 == 0 => DIRT_FULL_A,
        15 => DIRT_FULL_B,
        _ => grass_index(variant),
    }
}

fn water_tile_index(neighbors: TileNeighbors, variant: u64) -> usize {
    let cardinal_count = [
        neighbors.contains(TileNeighbors::N),
        neighbors.contains(TileNeighbors::E),
        neighbors.contains(TileNeighbors::S),
        neighbors.contains(TileNeighbors::W),
    ]
    .into_iter()
    .filter(|same| *same)
    .count();
    if cardinal_count == 0 {
        if variant & 1 == 0 {
            WATER_ISOLATED
        } else {
            WATER_ISOLATED_SMALL
        }
    } else if cardinal_count == 1 && neighbors.contains(TileNeighbors::E) {
        WATER_CAP_W
    } else if cardinal_count == 1 && neighbors.contains(TileNeighbors::W) {
        WATER_CAP_E
    } else {
        transition_index(WATER_TILES, neighbors, variant)
    }
}

fn cell_position(x: usize, y: usize) -> Vec2 {
    Vec2::new(
        (x as f32).mul_add(TILE_SIZE, -MAP_HALF_WIDTH + TILE_SIZE / 2.0),
        (y as f32).mul_add(TILE_SIZE, -MAP_HALF_HEIGHT + TILE_SIZE / 2.0),
    )
}

fn intersection_position(x: usize, y: usize) -> Vec2 {
    Vec2::new(
        ((x as f32) - 1.0).mul_add(TILE_SIZE, -MAP_HALF_WIDTH),
        ((y as f32) - 1.0).mul_add(TILE_SIZE, -MAP_HALF_HEIGHT),
    )
}

fn camera_cell(position: Vec2) -> (usize, usize) {
    let x = ((position.x + MAP_HALF_WIDTH) / TILE_SIZE)
        .floor()
        .clamp(0.0, MAP_WIDTH as f32 - 1.0) as usize;
    let y = ((position.y + MAP_HALF_HEIGHT) / TILE_SIZE)
        .floor()
        .clamp(0.0, MAP_HEIGHT as f32 - 1.0) as usize;
    (x, y)
}

fn dual_grid_debug_label(x: usize, y: usize, mask: DualGridMask, atlas_index: usize) -> String {
    let info = TILE_DEBUG_INFO[atlas_index];
    format!(
        "R{x},{y} M{:02X}\n{}{}/{}{}\nA{atlas_index} G{},{}\n{}",
        mask.0,
        mask.corner(DualGridMask::NW),
        mask.corner(DualGridMask::NE),
        mask.corner(DualGridMask::SW),
        mask.corner(DualGridMask::SE),
        info.source_column,
        info.source_row,
        info.role,
    )
}

const fn terrain_code(terrain: Terrain) -> char {
    match terrain {
        Terrain::Grass => 'G',
        Terrain::Dirt => 'D',
        Terrain::Water => 'W',
    }
}

pub fn spawn_debug_legend(commands: &mut Commands) {
    commands.spawn((
        Text::new(
            "TERRAIN DEBUG · F3 to hide\n\
             W{x,y}: logical cell · R{x,y}: dual-grid rendered tile\n\
             M## and DD/GG: dirt mask (NW NE / SW SE)\n\
             A##: runtime slot · G{x,y}: source tile\n\
             W/R origin: bottom-left · G origin: top-left",
        ),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        TextColor(Color::WHITE),
        TextBackgroundColor(Color::srgba(0.02, 0.02, 0.02, 0.88)),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(12.0),
            top: Val::Px(12.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
        ZIndex(100),
        Visibility::Hidden,
        TerrainDebugLegend,
    ));
}

pub fn update_debug_overlay(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    grid: Res<WorldGrid>,
    camera: Query<&Transform, With<Camera2d>>,
    labels: Query<Entity, With<TerrainDebugLabel>>,
    mut legend: Query<&mut Visibility, With<TerrainDebugLegend>>,
    mut state: ResMut<TerrainDebugOverlay>,
) {
    if keys.just_pressed(KeyCode::F3) {
        state.enabled = !state.enabled;
        state.rendered_center = None;
        if let Ok(mut visibility) = legend.single_mut() {
            *visibility = if state.enabled {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }

    if !state.enabled {
        if !labels.is_empty() {
            for entity in &labels {
                commands.entity(entity).despawn();
            }
        }
        return;
    }

    let Ok(camera) = camera.single() else {
        return;
    };
    let center = camera_cell(camera.translation.truncate());
    if state.rendered_center == Some(center) {
        return;
    }
    for entity in &labels {
        commands.entity(entity).despawn();
    }

    let min_x = center.0.saturating_sub(DEBUG_HALF_COLUMNS);
    let max_x = (center.0 + DEBUG_HALF_COLUMNS).min(MAP_WIDTH - 1);
    let min_y = center.1.saturating_sub(DEBUG_HALF_ROWS);
    let max_y = (center.1 + DEBUG_HALF_ROWS).min(MAP_HEIGHT - 1);
    for y in min_y + 1..=max_y + 2 {
        for x in min_x + 1..=max_x + 2 {
            let mask = DualGridMask::at_intersection(&grid, x, y);
            let variant = cell_hash(WORLD_SEED ^ 0xA71A, x as u64, y as u64);
            let atlas_index = dual_grid_index(mask, variant);
            let position = intersection_position(x, y);
            commands.spawn((
                Text2d::new(dual_grid_debug_label(x, y, mask, atlas_index)),
                TextFont {
                    font_size: 4.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                TextLayout::new_with_justify(Justify::Center),
                TextBackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.68)),
                Text2dShadow {
                    offset: Vec2::new(1.0, -1.0),
                    color: Color::BLACK,
                },
                Transform::from_xyz(position.x, position.y, 20.0),
                TerrainDebugLabel,
            ));
        }
    }
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let terrain = grid.get(x, y);
            let position = cell_position(x, y);
            commands.spawn((
                Text2d::new(format!("W{x},{y} {}", terrain_code(terrain))),
                TextFont {
                    font_size: 4.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.82, 0.3)),
                TextLayout::new_with_justify(Justify::Center),
                Text2dShadow {
                    offset: Vec2::new(1.0, -1.0),
                    color: Color::BLACK,
                },
                Transform::from_xyz(position.x, position.y, 21.0),
                TerrainDebugLabel,
            ));
        }
    }
    state.rendered_center = Some(center);
}

pub fn spawn_terrain(
    commands: &mut Commands,
    asset_server: &AssetServer,
    texture_atlas_layouts: &mut Assets<TextureAtlasLayout>,
) -> WorldGrid {
    let grid = WorldGrid::generate(WORLD_SEED);
    let texture = asset_server.load("world/terrain.png");
    let layout =
        TextureAtlasLayout::from_grid(UVec2::splat(TILE_SIZE as u32), ATLAS_COLUMNS, 1, None, None);
    let layout = texture_atlas_layouts.add(layout);
    for y in 1..=MAP_HEIGHT + 1 {
        for x in 1..=MAP_WIDTH + 1 {
            let mask = DualGridMask::at_intersection(&grid, x, y);
            let variant = cell_hash(WORLD_SEED ^ 0xA71A, x as u64, y as u64);
            let index = dual_grid_index(mask, variant);
            let position = intersection_position(x, y);
            commands.spawn((
                Sprite {
                    image: texture.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: layout.clone(),
                        index,
                    }),
                    ..default()
                },
                Transform::from_xyz(position.x, position.y, -10.0),
            ));
        }
    }
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if grid.get(x, y) != Terrain::Water {
                continue;
            }
            let variant = cell_hash(WORLD_SEED ^ 0xA71A, x as u64, y as u64);
            let index = water_tile_index(grid.neighbors(x, y, Terrain::Water), variant);
            let position = cell_position(x, y);
            commands.spawn((
                Sprite {
                    image: texture.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: layout.clone(),
                        index,
                    }),
                    ..default()
                },
                Transform::from_xyz(position.x, position.y, -9.0),
            ));
        }
    }

    info!(
        "Terrain initialized: {}x{} cells from seed {WORLD_SEED:#x}",
        MAP_WIDTH, MAP_HEIGHT
    );
    grid
}

const fn cell_hash(seed: u64, x: u64, y: u64) -> u64 {
    let mut value =
        seed ^ x.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ y.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn lattice(seed: u64, x: i32, y: i32) -> f32 {
    (cell_hash(seed, i64::from(x) as u64, i64::from(y) as u64) >> 40) as f32 / (1_u64 << 24) as f32
}

fn value_noise(seed: u64, x: f32, y: f32) -> f32 {
    let x0 = x.floor();
    let y0 = y.floor();
    let tx = smoothstep(x - x0);
    let ty = smoothstep(y - y0);
    let (x0, y0) = (x0 as i32, y0 as i32);
    let v00 = lattice(seed, x0, y0);
    let v10 = lattice(seed, x0 + 1, y0);
    let v01 = lattice(seed, x0, y0 + 1);
    let v11 = lattice(seed, x0 + 1, y0 + 1);
    let top = (v10 - v00).mul_add(tx, v00);
    let bottom = (v11 - v01).mul_add(tx, v01);
    (bottom - top).mul_add(ty, top)
}

fn smoothstep(value: f32) -> f32 {
    value * value * value.mul_add(-2.0, 3.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_reported_area_before_checkerboard_cleanup() {
        let mut grid = WorldGrid {
            cells: vec![Terrain::Grass; MAP_WIDTH * MAP_HEIGHT],
        };
        grid.grow_dirt(WORLD_SEED);
        grid.remove_dirt_speckles();
        grid.clear_waystation();
        grid.lay_old_road();
        grid.add_water(WORLD_SEED);
        grid.enforce_grass_shores();
        for y in (20..=24).rev() {
            let row: String = (20..=25).map(|x| terrain_code(grid.get(x, y))).collect();
            println!("before y={y}: {row}");
        }
        grid.resolve_dirt_checkerboards(WORLD_SEED);
        for y in (20..=24).rev() {
            let row: String = (20..=25).map(|x| terrain_code(grid.get(x, y))).collect();
            println!("after  y={y}: {row}");
        }
    }

    #[test]
    fn generation_is_deterministic_and_seeded() {
        assert_eq!(WorldGrid::generate(42), WorldGrid::generate(42));
        assert_ne!(WorldGrid::generate(42), WorldGrid::generate(43));
    }

    #[test]
    fn generated_water_has_a_grass_shore() {
        let grid = WorldGrid::generate(WORLD_SEED);
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                if grid.get(x, y) != Terrain::Water {
                    continue;
                }
                for &(dx, dy) in &NEIGHBOR_OFFSETS {
                    let nx = x as isize + dx;
                    let ny = y as isize + dy;
                    if nx < 0 || ny < 0 || nx >= MAP_WIDTH as isize || ny >= MAP_HEIGHT as isize {
                        continue;
                    }
                    assert_ne!(grid.get(nx as usize, ny as usize), Terrain::Dirt);
                }
            }
        }
    }

    #[test]
    fn world_footprints_make_water_and_map_edges_impassable() {
        let grid = WorldGrid::generate(WORLD_SEED);
        let water = (0..MAP_HEIGHT)
            .flat_map(|y| (0..MAP_WIDTH).map(move |x| (x, y)))
            .find(|&(x, y)| grid.get(x, y) == Terrain::Water)
            .expect("generated world has water");
        let water_position = cell_position(water.0, water.1);

        assert!(!grid.supports_land_footprint(water_position, Vec2::splat(1.0)));
        assert!(!grid.supports_land_footprint(Vec2::new(MAP_HALF_WIDTH, 0.0), Vec2::splat(1.0)));
    }

    #[test]
    fn land_footprints_reject_any_overlapped_water_cell() {
        let grid = WorldGrid::generate(WORLD_SEED);
        let water = (0..MAP_HEIGHT)
            .flat_map(|y| (0..MAP_WIDTH).map(move |x| (x, y)))
            .find(|&(x, y)| grid.get(x, y) == Terrain::Water)
            .expect("generated world has water");
        let water_position = cell_position(water.0, water.1);

        assert!(!grid.supports_land_footprint(water_position, Vec2::splat(1.0)));
    }

    #[test]
    fn water_transition_lookup_keeps_inner_corners() {
        let inner_nw = TileNeighbors(!TileNeighbors::NW);
        assert_eq!(water_tile_index(inner_nw, 0), WATER_INNER_NW);
    }

    #[test]
    fn isolated_water_uses_the_special_art() {
        let index = water_tile_index(TileNeighbors::default(), 0);
        assert_eq!(index, WATER_ISOLATED);
    }

    #[test]
    fn generated_ground_has_no_unsupported_checkerboard_masks() {
        let grid = WorldGrid::generate(WORLD_SEED);
        for y in 1..=MAP_HEIGHT + 1 {
            for x in 1..=MAP_WIDTH + 1 {
                assert!(!DualGridMask::at_intersection(&grid, x, y).is_checkerboard());
            }
        }
    }

    #[test]
    fn three_dirt_cells_at_a_shared_corner_use_the_matching_transition() {
        let mut grid = WorldGrid {
            cells: vec![Terrain::Grass; MAP_WIDTH * MAP_HEIGHT],
        };
        let (x, y) = (MAP_WIDTH / 2, MAP_HEIGHT / 2);
        grid.set(x - 1, y, Terrain::Dirt);
        grid.set(x, y - 1, Terrain::Dirt);
        grid.set(x - 1, y - 1, Terrain::Dirt);

        let mask = DualGridMask::at_intersection(&grid, x + 1, y + 1);
        assert_eq!(mask, DualGridMask(13));
        assert_eq!(dual_grid_index(mask, 0), DIRT_EXCEPT_NE);
    }

    #[test]
    fn diagonal_staircase_masks_keep_convex_corners_grassy() {
        for (mask, index, source) in [(2, GRASS_PLAIN, (0, 0)), (7, DIRT_EXCEPT_SW, (5, 2))] {
            let actual_index = dual_grid_index(DualGridMask(mask), 0);
            let info = TILE_DEBUG_INFO[actual_index];
            assert_eq!(actual_index, index);
            assert_eq!((info.source_column, info.source_row), source);
        }
    }

    #[test]
    fn rendered_grid_is_offset_half_a_tile_from_logical_cells() {
        assert_eq!(
            cell_position(0, 0) - intersection_position(1, 1),
            Vec2::splat(TILE_SIZE / 2.0)
        );
    }

    #[test]
    fn dual_grid_catalog_covers_every_supported_dirt_mask() {
        assert_eq!(TILE_DEBUG_INFO.len(), ATLAS_COLUMNS as usize);
        for mask in [1, 2, 4, 8, 5, 10] {
            assert_eq!(dual_grid_index(DualGridMask(mask), 0), GRASS_PLAIN);
        }
        for (mask, index, source) in [
            (3, DIRT_NORTH, (6, 2)),
            (6, DIRT_EAST, (7, 2)),
            (12, DIRT_SOUTH, (6, 3)),
            (9, DIRT_WEST, (7, 3)),
            (14, DIRT_EXCEPT_NW, (5, 3)),
            (13, DIRT_EXCEPT_NE, (4, 3)),
            (11, DIRT_EXCEPT_SE, (4, 2)),
            (7, DIRT_EXCEPT_SW, (5, 2)),
            (15, DIRT_FULL_A, (0, 2)),
        ] {
            let info = TILE_DEBUG_INFO[index];
            assert_eq!(dual_grid_index(DualGridMask(mask), 0), index);
            assert_eq!((info.source_column, info.source_row), source);
        }
        assert_eq!(TILE_DEBUG_INFO[WATER_INNER_SW].source_column, 39);
        assert_eq!(
            dual_grid_debug_label(30, 25, DualGridMask(13), DIRT_EXCEPT_NE),
            "R30,25 M0D\nDG/DD\nA15 G4,3\nDG.except_ne"
        );
    }
}
