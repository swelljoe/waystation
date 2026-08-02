//! Drawing a generated traveller.
//!
//! `waystation-npcgen` decides who walks down the road and hands back a list of
//! sprite sheets to stack, in order, each with the palette swap it needs. This
//! stacks them. It is the only part of the pipeline that knows what a pixel is.
//!
//! Compositing happens here rather than at build time because that is the whole
//! point: a game that bakes its travellers has as many strangers as somebody
//! remembered to bake, and a waystation that keeps its fire lit for two hundred
//! days needs more than that.
//!
//! It also happens *only* here. Composited travellers are LPC art, some of it
//! share-alike, and they exist as a texture in memory for as long as somebody is
//! standing in the court. They are never written to disk and never flattened
//! into an image with the purchased tilesets around them, which is exactly the
//! distinction share-alike cares about.

use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use waystation_npcgen::{Draw, Npc};

/// Where `scripts/build-npc-art.py` puts the sheets, under the asset root.
const ART_ROOT: &str = "npc";

/// The geometry of an LPC walk sheet: nine frames across, one row per facing.
pub const FRAME: u32 = 64;
pub const COLUMNS: u32 = 9;
pub const ROWS: u32 = 4;

/// A body whose sheets are still loading.
///
/// Kept on the entity rather than in a side table so that a visitor who leaves
/// mid-load takes their pending work with them.
#[derive(Component)]
pub struct ComposingBody {
    draws: Vec<Draw>,
    sheets: Vec<Handle<Image>>,
}

impl ComposingBody {
    /// Ask for every sheet this traveller is drawn from.
    pub fn request(npc: &Npc, asset_server: &AssetServer) -> Self {
        let draws = npc.draws();
        let sheets = draws
            .iter()
            .map(|draw| asset_server.load(format!("{ART_ROOT}/{}", draw.sheet)))
            .collect();
        Self { draws, sheets }
    }
}

/// Whether every sheet has arrived, or one of them never will.
enum Readiness {
    Waiting,
    Ready,
    /// At least one sheet failed to load. The traveller keeps the fallback art
    /// they were spawned with rather than staying invisible forever.
    Lost,
}

fn readiness(body: &ComposingBody, asset_server: &AssetServer) -> Readiness {
    let mut ready = true;
    for handle in &body.sheets {
        match asset_server.get_load_state(handle) {
            Some(bevy::asset::LoadState::Loaded) => {}
            Some(bevy::asset::LoadState::Failed(_)) => return Readiness::Lost,
            _ => ready = false,
        }
    }
    if ready {
        Readiness::Ready
    } else {
        Readiness::Waiting
    }
}

/// Swaps a composited sheet onto every visitor whose art has finished loading.
pub fn compose_visitor_art(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    waiting: Query<(Entity, &ComposingBody)>,
    mut sprites: Query<&mut Sprite>,
) {
    for (entity, body) in &waiting {
        let composed = match readiness(body, &asset_server) {
            Readiness::Waiting => continue,
            Readiness::Lost => {
                warn!("a visitor's art failed to load; falling back to a hand-made sheet");
                None
            }
            Readiness::Ready => {
                let layers: Vec<(&Draw, &Image)> = body
                    .draws
                    .iter()
                    .zip(&body.sheets)
                    .filter_map(|(draw, handle)| Some((draw, images.get(handle)?)))
                    .collect();
                compose(&layers)
            }
        };
        if let Some(sheet) = composed {
            if let Ok(mut sprite) = sprites.get_mut(entity) {
                sprite.image = images.add(sheet);
            }
        }
        commands.entity(entity).remove::<ComposingBody>();
    }
}

/// Stack loaded sheets into one, recolouring each on the way up.
///
/// `None` when there is nothing to draw, or when the first layer is not a plain
/// 8-bit RGBA sheet — every LPC sheet is, so that is a bug rather than a case
/// to paper over.
#[must_use]
pub fn compose(layers: &[(&Draw, &Image)]) -> Option<Image> {
    let (_, first) = layers.first()?;
    let (width, height) = (first.width(), first.height());
    let mut canvas = vec![0_u8; (width as usize) * (height as usize) * 4];

    for (draw, sheet) in layers {
        if sheet.texture_descriptor.format != TextureFormat::Rgba8UnormSrgb {
            warn!("{}: not 8-bit RGBA, skipping", draw.sheet);
            continue;
        }
        if sheet.width() != width || sheet.height() != height {
            warn!(
                "{}: {}x{} among {width}x{height} sheets, skipping",
                draw.sheet,
                sheet.width(),
                sheet.height()
            );
            continue;
        }
        let Some(data) = sheet.data.as_ref() else {
            warn!("{}: no pixels in main world, skipping", draw.sheet);
            continue;
        };
        for (under, over) in canvas.chunks_exact_mut(4).zip(data.chunks_exact(4)) {
            let source = swap(&draw.swap, [over[0], over[1], over[2]], over[3]);
            let blended = over_under(source, [under[0], under[1], under[2], under[3]]);
            under.copy_from_slice(&blended);
        }
    }

    let mut composed = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        canvas,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    // Pixel art enlarged four times: anything but nearest turns a 64-pixel
    // person into a smudge.
    composed.sampler = ImageSampler::nearest();
    Some(composed)
}

/// A pixel's colour after its piece's palette swap.
///
/// Transparent pixels are left alone: LPC sheets carry rubbish in the colour
/// channels behind full transparency, and swapping it would be work with no
/// visible result.
fn swap(table: &[([u8; 3], [u8; 3])], color: [u8; 3], alpha: u8) -> [u8; 4] {
    if alpha == 0 {
        return [color[0], color[1], color[2], 0];
    }
    for (from, to) in table {
        if *from == color {
            return [to[0], to[1], to[2], alpha];
        }
    }
    [color[0], color[1], color[2], alpha]
}

/// Extra bits of headroom the blend carries so its rounding lands where
/// Pillow's does. Both this and the `/255` below are Pillow's own, deliberately.
const PRECISION_BITS: u32 = 7;

/// Divide by 255, rounding the way Pillow rounds.
const fn div255(value: u32) -> u32 {
    ((value >> 8) + value) >> 8
}

/// Source-over compositing, integer for integer with Pillow's
/// `Image.alpha_composite`.
///
/// Matching a specific implementation looks fussy until you ask what it buys:
/// `scripts/preview-npcs.py` composites with Pillow and its output has been
/// checked byte-for-byte against sheets the LPC web app exported itself. Being
/// exactly equal to Pillow therefore means being exactly equal to the app, and
/// the reference sheets in `runtime-assets/npc/reference` can be compared with
/// `==` instead of a tolerance that would hide a real disagreement.
fn over_under(source: [u8; 4], under: [u8; 4]) -> [u8; 4] {
    let (source_alpha, under_alpha) = (u32::from(source[3]), u32::from(under[3]));
    if source_alpha == 0 {
        return under;
    }
    let out_alpha = source_alpha * 255 + under_alpha * (255 - source_alpha);
    let over_coefficient = source_alpha * 255 * 255 * (1 << PRECISION_BITS) / out_alpha;
    let under_coefficient = 255 * (1 << PRECISION_BITS) - over_coefficient;
    let channel = |index: usize| {
        let mixed = u32::from(source[index]) * over_coefficient
            + u32::from(under[index]) * under_coefficient
            + (0x80 << PRECISION_BITS);
        #[allow(clippy::cast_possible_truncation)]
        {
            (div255(mixed) >> PRECISION_BITS) as u8
        }
    };
    #[allow(clippy::cast_possible_truncation)]
    [
        channel(0),
        channel(1),
        channel(2),
        div255(out_alpha + 0x80) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet(pixels: &[[u8; 4]]) -> Image {
        #[allow(clippy::cast_possible_truncation)]
        let width = pixels.len() as u32;
        Image::new(
            Extent3d {
                width,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels.iter().flatten().copied().collect(),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD,
        )
    }

    fn draw(swap: Vec<([u8; 3], [u8; 3])>) -> Draw {
        Draw {
            sheet: "test/walk.png".to_owned(),
            z: 0,
            swap,
        }
    }

    fn pixels(image: &Image) -> Vec<[u8; 4]> {
        image
            .data
            .as_ref()
            .expect("composed image keeps its pixels")
            .chunks_exact(4)
            .map(|chunk| [chunk[0], chunk[1], chunk[2], chunk[3]])
            .collect()
    }

    #[test]
    fn an_opaque_layer_hides_what_is_under_it() {
        let under = sheet(&[[10, 20, 30, 255]]);
        let over = sheet(&[[90, 80, 70, 255]]);
        let composed = compose(&[(&draw(vec![]), &under), (&draw(vec![]), &over)])
            .expect("two layers compose");

        assert_eq!(pixels(&composed), vec![[90, 80, 70, 255]]);
    }

    #[test]
    fn a_transparent_pixel_leaves_what_is_under_it_alone() {
        let under = sheet(&[[10, 20, 30, 255]]);
        let over = sheet(&[[99, 99, 99, 0]]);
        let composed =
            compose(&[(&draw(vec![]), &under), (&draw(vec![]), &over)]).expect("composes");

        assert_eq!(pixels(&composed), vec![[10, 20, 30, 255]]);
    }

    /// The cast shadow LPC draws on a forehead is flat black at partial alpha,
    /// and it has to darken the face rather than replace it.
    #[test]
    fn a_partial_alpha_shadow_darkens_rather_than_covers() {
        let under = sheet(&[[200, 180, 160, 255]]);
        let shadow = sheet(&[[0, 0, 0, 64]]);
        let composed =
            compose(&[(&draw(vec![]), &under), (&draw(vec![]), &shadow)]).expect("composes");

        let [red, green, blue, alpha] = pixels(&composed)[0];
        assert_eq!(alpha, 255, "the face is still opaque");
        assert!(red < 200 && green < 180 && blue < 160, "nothing darkened");
        assert!(red > 100, "the shadow swallowed the face");
    }

    /// Every value here was produced by Pillow's own `alpha_composite`. If this
    /// fails, the reference sheets stop being comparable byte-for-byte.
    #[test]
    fn the_blend_agrees_with_pillow_pixel_for_pixel() {
        for (source, under, expected) in [
            ([0, 0, 0, 64], [200, 180, 160, 255], [150, 135, 120, 255]),
            ([255, 255, 255, 89], [38, 13, 20, 255], [114, 97, 102, 255]),
            ([164, 38, 0, 128], [7, 200, 13, 255], [86, 119, 6, 255]),
            ([200, 100, 50, 112], [90, 60, 40, 200], [145, 80, 45, 224]),
            ([1, 2, 3, 255], [90, 60, 40, 0], [1, 2, 3, 255]),
            ([1, 2, 3, 10], [90, 60, 40, 0], [1, 2, 3, 10]),
        ] {
            assert_eq!(
                over_under(source, under),
                expected,
                "{source:?} over {under:?}"
            );
        }
    }

    #[test]
    fn a_swap_repaints_only_the_colours_it_names() {
        let table = vec![([10, 20, 30], [77, 88, 99])];
        let under = sheet(&[[10, 20, 30, 255], [40, 50, 60, 255], [10, 20, 30, 0]]);
        let composed = compose(&[(&draw(table), &under)]).expect("composes");

        assert_eq!(
            pixels(&composed),
            vec![[77, 88, 99, 255], [40, 50, 60, 255], [0, 0, 0, 0]],
            "a named colour is repainted, an unnamed one is not, and \
             a transparent one stays transparent"
        );
    }

    #[test]
    fn nothing_to_draw_composes_to_nothing() {
        assert!(compose(&[]).is_none());
    }

    // --- Against the reference renderer ------------------------------------

    fn runtime_art() -> std::path::PathBuf {
        std::path::Path::new("runtime-assets").join(ART_ROOT)
    }

    fn decode(path: &std::path::Path) -> Image {
        let bytes = std::fs::read(path).unwrap_or_else(|why| panic!("{}: {why}", path.display()));
        Image::from_buffer(
            &bytes,
            bevy::image::ImageType::Extension("png"),
            bevy::image::CompressedImageFormats::NONE,
            true,
            ImageSampler::nearest(),
            RenderAssetUsages::MAIN_WORLD,
        )
        .unwrap_or_else(|why| panic!("{}: {why}", path.display()))
    }

    #[derive(serde::Deserialize)]
    struct ReferencePiece {
        slot: String,
        item: String,
        #[serde(default)]
        color: String,
    }

    #[derive(serde::Deserialize)]
    struct ReferenceCharacter {
        name: String,
        body: String,
        pieces: Vec<ReferencePiece>,
    }

    /// The check that the game draws travellers the way the LPC web app does.
    ///
    /// `scripts/build-npc-art.py` composites a fixed cast with Pillow, from the
    /// same wardrobe record this reads, and Pillow's compositing has already
    /// been shown byte-for-byte equal to sheets the web app exported itself.
    /// Two independent implementations of layer order, `${head}` substitution
    /// and palette recolouring have to agree exactly, or a traveller here is
    /// wearing something the tool never put on them.
    #[test]
    fn every_reference_traveller_composes_to_the_same_pixels() {
        let reference = runtime_art().join("reference");
        let cast = reference.join("cast.json");
        if !cast.is_file() {
            eprintln!(
                "no {} — generated visitor art was not built, so the game's \
                 compositor is unchecked against the reference renderer. \
                 Run `make assets` with an LPC checkout to cover it.",
                cast.display()
            );
            return;
        }

        let people: Vec<ReferenceCharacter> =
            serde_json::from_str(&std::fs::read_to_string(&cast).expect("cast.json is readable"))
                .expect("cast.json is valid");
        assert!(!people.is_empty(), "the reference cast is empty");

        for person in &people {
            let chosen: Vec<(&str, &str, &str)> = person
                .pieces
                .iter()
                .map(|piece| {
                    (
                        piece.slot.as_str(),
                        piece.item.as_str(),
                        piece.color.as_str(),
                    )
                })
                .collect();
            let npc = waystation_npcgen::Npc::assembled(&person.body, &chosen)
                .unwrap_or_else(|why| panic!("{}: {why}", person.name));

            let draws = npc.draws();
            assert!(!draws.is_empty(), "{} draws nothing", person.name);
            let sheets: Vec<Image> = draws
                .iter()
                .map(|draw| decode(&runtime_art().join(&draw.sheet)))
                .collect();
            let layers: Vec<(&Draw, &Image)> = draws.iter().zip(&sheets).collect();
            let composed = compose(&layers).expect("a traveller with layers composes");

            let expected = decode(&reference.join(format!("{}.png", person.name)));
            assert_eq!(
                composed.width(),
                expected.width(),
                "{}: wrong sheet width",
                person.name
            );
            assert_eq!(
                composed.data, expected.data,
                "{} is drawn differently here than by scripts/build-npc-art.py",
                person.name
            );
        }
    }
}
