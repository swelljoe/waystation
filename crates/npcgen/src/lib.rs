//! Everybody else who walks down the road.
//!
//! The waystation gets strangers for as long as the fire keeps burning, and
//! four hand-drawn sheets run out long before the game does. This builds new
//! ones: a body, a face that suits it, and clothes that look like they came out
//! of a valley rather than off a fantasy rack.
//!
//! The output is a set of Universal LPC Spritesheet Generator selections, in
//! that app's own `character.json` shape. That is deliberate — a traveller the
//! generator produced can be pasted straight into the web tool, looked at,
//! adjusted by hand, and exported as a finished sheet. When something comes out
//! wrong, the fix is visible rather than guessed at.
//!
//! This crate does not draw anything itself — it has no business knowing what
//! an image is — but it does say exactly what to draw. [`Npc::draws`] turns a
//! traveller into an ordered list of sprite sheets and palette swaps, which is
//! everything a compositor needs and nothing it does not.

pub mod wardrobe;

use std::fmt::Write as _;

use wardrobe::{ColorField, ColorSource, Item, Wardrobe};

/// How much colour the world can still make.
///
/// Early on, everything anyone wears was scavenged or dyed with what grows in a
/// dry valley. The story later brings real dye back, and travellers start
/// arriving in colours that cost something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Era {
    /// Browns, tans, black, undyed white, and the greens and oranges a plant
    /// dye can reach.
    #[default]
    Scavenged,
    /// The above, plus the reds, blues and yellows that mean a working trade.
    Dyed,
}

/// What the art a traveller is made of allows.
///
/// LPC offers most of its art under several licences at once. Waystation never
/// takes the GPL offer, so every piece here is usable — but for some the only
/// remaining offer is CC-BY-SA, which is share-alike. That is fine inside a
/// running game, and it is a problem in a flat image: a screenshot or trailer
/// frame mixes travellers with purchased tilesets that cannot be relicensed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArtLicense {
    /// Every piece has a CC0, CC-BY or OGA-BY offer. Credit the artists and the
    /// sheet is otherwise unencumbered — safe to flatten into a screenshot
    /// alongside art under any other licence.
    AttributionOnly,
    /// At least one piece is CC-BY-SA with no plainer offer, so the composited
    /// sheet is share-alike too. Fine in-game; do not bake it into an image
    /// with art you cannot license the same way.
    #[default]
    ShareAlike,
}

/// Who a caller wants, when they want something in particular.
///
/// The game's arrivals are authored as social shapes — someone walking alone,
/// two siblings, an old hand who has seen this road before — and those shapes
/// need people of roughly the right age standing in them. This is the whole of
/// the constraint: everything else about the traveller is still rolled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Cast {
    /// No constraint. Any base, any age.
    #[default]
    Anyone,
    /// A grown adult who does not read as old.
    Grown,
    /// An old face, with the grey hair and the likely cane that follow from it.
    Elder,
    /// The `teen` base: old enough to walk a road alone, young enough that
    /// doing so reads as a risk.
    Youth,
    Child,
}

impl Cast {
    /// Body bases this casting will accept.
    const fn bases(self) -> &'static [&'static str] {
        match self {
            Self::Anyone => &[],
            Self::Grown | Self::Elder => &["male", "female"],
            Self::Youth => &["teen"],
            Self::Child => &["child"],
        }
    }

    fn allows_base(self, id: &str) -> bool {
        let bases = self.bases();
        bases.is_empty() || bases.contains(&id)
    }

    /// Whether an elderly head is required, forbidden, or simply rolled.
    const fn elderly(self) -> Option<bool> {
        match self {
            Self::Anyone => None,
            Self::Elder => Some(true),
            Self::Grown | Self::Youth | Self::Child => Some(false),
        }
    }
}

/// Everything about a traveller that is asked for rather than rolled.
#[derive(Debug, Clone, Copy, Default)]
pub struct Casting {
    pub era: Era,
    /// The licence bar. See [`ArtLicense`].
    pub license: ArtLicense,
    pub cast: Cast,
}

/// Licence families that carry no copyleft obligation.
const UNENCUMBERED: [&str; 3] = ["CC0", "CC-BY", "OGA-BY"];

/// One piece of a traveller, in the shape the LPC app stores it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// The LPC `type_name`. Also the key this piece occupies in `selections`,
    /// which is what makes two hats impossible.
    pub slot: String,
    pub item_id: String,
    /// Display name including the colour, e.g. `Basic Shoes (tan)`.
    pub name: String,
    /// Set for pieces that ship one baked sheet per colour.
    pub variant: String,
    /// Set for pieces recoloured against a palette at draw time.
    pub recolor: String,
    /// The piece's own name without the colour, kept for the URL hash.
    base_name: String,
    /// What this piece's art allows on its own.
    pub license: ArtLicense,
}

impl Piece {
    /// The colour, whichever field it landed in.
    #[must_use]
    pub fn color(&self) -> &str {
        if self.variant.is_empty() {
            &self.recolor
        } else {
            &self.variant
        }
    }
}

/// A generated traveller.
#[derive(Debug, Clone)]
pub struct Npc {
    /// The LPC body base: `male`, `female`, `muscular`, `teen`, `child`,
    /// `pregnant`.
    pub body_type: String,
    pub skin: String,
    pub hair_color: String,
    /// Which head family this traveller reads as. Rolled separately from the
    /// body base, because the `teen` base is a silhouette rather than a
    /// biography and LPC draws no second one.
    pub presents_masc: bool,
    /// True once an elderly head is chosen, which pulls grey into the hair and
    /// makes lines, an old nose and a cane likelier.
    pub elderly: bool,
    pub era: Era,
    /// The licence bar this traveller was generated against. `AttributionOnly`
    /// means share-alike art was refused outright rather than merely reported.
    pub required: ArtLicense,
    /// Who was asked for. Kept because it constrains choices still being made
    /// while the traveller is built, and because it explains the result.
    pub cast: Cast,
    /// The seed this traveller came from. Enough to rebuild them exactly.
    pub seed: u64,
    /// In drawing order, roughly bottom to top.
    pub pieces: Vec<Piece>,
}

impl Npc {
    /// Bases that should never be handed an old face or an old beard.
    fn young(&self) -> bool {
        matches!(self.body_type.as_str(), "teen" | "child")
    }

    /// What the finished sheet allows.
    ///
    /// One share-alike piece makes the whole composite share-alike — the
    /// obligation attaches to the adaptation, and a traveller is one image.
    #[must_use]
    pub fn art_license(&self) -> ArtLicense {
        if self
            .pieces
            .iter()
            .any(|p| p.license == ArtLicense::ShareAlike)
        {
            ArtLicense::ShareAlike
        } else {
            ArtLicense::AttributionOnly
        }
    }

    /// The pieces responsible for a share-alike sheet, for when you want to
    /// know what to swap rather than just that it is encumbered.
    #[must_use]
    pub fn encumbered_pieces(&self) -> Vec<&Piece> {
        self.pieces
            .iter()
            .filter(|p| p.license == ArtLicense::ShareAlike)
            .collect()
    }
}

/// A small stirred pseudo-random source, matching the one the game already
/// uses. Kept local so the generator has no dependency that needs teaching how
/// to find entropy inside a `WebAssembly` sandbox, and so a seed reproduces a
/// traveller exactly.
struct Rng {
    state: u64,
}

const SEED_MIX: u64 = 0x9E37_79B9_7F4A_7C15;

impl Rng {
    const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { SEED_MIX } else { seed },
        }
    }

    const fn next(&mut self) -> u64 {
        // xorshift64*, which is short, has no bad seeds beyond zero, and is far
        // better than anything this game can tell the difference from.
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        if self.state == 0 {
            self.state = SEED_MIX;
        }
        self.state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn odds(&mut self, probability: f32) -> bool {
        #[allow(clippy::cast_precision_loss)]
        let roll = (self.next() >> 40) as f32 / 16_777_216.0;
        roll < probability.clamp(0.0, 1.0)
    }

    /// Weighted choice. `None` only when every candidate weighs nothing, which
    /// is how a slot with no piece suitable for this traveller comes back empty.
    fn weighted<'a, T>(&mut self, items: &'a [T], weight: impl Fn(&T) -> u32) -> Option<&'a T> {
        let total: u64 = items.iter().map(|item| u64::from(weight(item))).sum();
        if total == 0 {
            return None;
        }
        let mut roll = self.next() % total;
        for item in items {
            let w = u64::from(weight(item));
            if roll < w {
                return Some(item);
            }
            roll -= w;
        }
        items.last()
    }
}

/// Which head family goes with a body base.
///
/// `male` and `female` bases have an obvious answer; `teen` and `child` do not,
/// because LPC draws one of each and expects the head above it to do the work.
/// Rolling here is what keeps teenage girls from being impossible.
fn presents_masc(rng: &mut Rng, body_type: &str) -> bool {
    match body_type {
        "female" | "pregnant" => false,
        "male" => true,
        _ => rng.odds(0.5),
    }
}

/// Old faces get old hair. Pieces marked as belonging with an old face are made
/// likelier rather than mandatory — a walrus moustache on a forty-year-old is
/// unusual, not impossible.
const ELDERLY_FAVOUR: u32 = 4;

/// Build a traveller from a seed.
///
/// The same seed and era always produce the same person, which is what makes a
/// generated cast reviewable: a rogue result can be reported by number.
#[must_use]
pub fn generate(seed: u64, era: Era) -> Npc {
    generate_for(seed, era, ArtLicense::ShareAlike)
}

/// Build a traveller of a particular age and licence bar.
///
/// The one call that constrains anything; [`generate`] and [`generate_for`] are
/// this with the constraint left off.
#[must_use]
pub fn generate_with(seed: u64, casting: Casting) -> Npc {
    build(seed, casting)
}

/// Build a traveller whose art meets a licence bar.
///
/// Pass [`ArtLicense::AttributionOnly`] for anyone who will end up in a flat
/// image — a screenshot, a trailer frame, store key art — where share-alike art
/// would meet purchased tilesets that cannot be relicensed to match. It costs
/// variety: no cane, no backpack, no scarf, and roughly a third of the
/// hairstyles and most of the noses go with them.
///
/// `ShareAlike` is not a request for share-alike art, it is the absence of a
/// restriction: it accepts everything, and the traveller may still come out
/// attribution-only. Ask [`Npc::art_license`] what you actually got.
#[must_use]
pub fn generate_for(seed: u64, era: Era, required: ArtLicense) -> Npc {
    build(
        seed,
        Casting {
            era,
            license: required,
            cast: Cast::Anyone,
        },
    )
}

fn build(seed: u64, casting: Casting) -> Npc {
    let wardrobe = Wardrobe::bundled();
    let mut rng = Rng::new(seed);

    let body_type = rng
        .weighted(&wardrobe.body_types, |b| {
            if casting.cast.allows_base(&b.id) {
                b.weight
            } else {
                0
            }
        })
        .map_or_else(|| "male".to_owned(), |b| b.id.clone());

    let skin = pick_color(&mut rng, &wardrobe.palettes.skin);
    let presents_masc = presents_masc(&mut rng, &body_type);

    let mut npc = Npc {
        skin,
        hair_color: String::new(),
        presents_masc,
        elderly: false,
        era: casting.era,
        required: casting.license,
        cast: casting.cast,
        seed,
        pieces: Vec::new(),
        body_type,
    };

    // The head has to come first: it decides whether this is an old face, and
    // everything from hair colour to whether a cane is likely follows from that.
    if let Some(item) = choose(&mut rng, wardrobe, "head", &npc) {
        npc.elderly = item.has_tag("elderly");
        push(&mut rng, &mut npc, item);
    }

    let hair_palette = if npc.elderly {
        &wardrobe.palettes.hair_old
    } else {
        &wardrobe.palettes.hair
    };
    npc.hair_color = pick_color(&mut rng, hair_palette);

    // Everything else. `bodies` in the wardrobe data already excludes pieces a
    // given base has no art for, so a child is left barefoot and beardless
    // without any rule here saying so.
    for slot in [
        "body",
        "hair",
        "beard",
        "mustache",
        "eyebrows",
        "expression",
        "nose",
        "wrinkles",
        "clothes",
        "legs",
        "shoes",
    ] {
        fill(&mut rng, &mut npc, wardrobe, slot);
    }

    // Choices the slot names cannot express on their own. An apron over
    // overalls is one garment too many, and a belt under suspenders is a joke.
    let workwear = one_of(&mut rng, &mut npc, wardrobe, &["apron", "overalls"]);
    if workwear != Some("overalls") {
        one_of(&mut rng, &mut npc, wardrobe, &["belt", "sash"]);
    }
    fill(&mut rng, &mut npc, wardrobe, "neck");
    if one_of(
        &mut rng,
        &mut npc,
        wardrobe,
        &["hat", "headcover", "bandana"],
    ) == Some("hat")
    {
        // A hood encloses the whole head. Hair drawn under one does not peek
        // out, it pushes through — a tied blonde ponytail standing up out of a
        // sack hood. A bandana or a headband sits in front of hair and is fine.
        npc.pieces.retain(|piece| piece.slot != "hair");
    }
    fill(&mut rng, &mut npc, wardrobe, "backpack");
    fill(&mut rng, &mut npc, wardrobe, "weapon");

    npc
}

/// What a single piece's art allows, from the licence families the wardrobe
/// recorded for it. An empty list would be a generation bug, and is treated as
/// encumbered rather than assumed safe.
fn license_of(item: &Item) -> ArtLicense {
    if item
        .licenses
        .iter()
        .any(|l| UNENCUMBERED.contains(&l.as_str()))
    {
        ArtLicense::AttributionOnly
    } else {
        ArtLicense::ShareAlike
    }
}

fn pick_color(rng: &mut Rng, palette: &[wardrobe::Weighted]) -> String {
    rng.weighted(palette, |c| c.weight)
        .map_or_else(String::new, |c| c.color.clone())
}

/// Weight a piece for this traveller, or zero it out entirely.
///
/// Zero means "not this person": wrong body base, wrong head family, a piece
/// that only belongs on an old face, or an old face on somebody too young for
/// one.
fn eligibility(item: &Item, npc: &Npc) -> u32 {
    if !item.fits(&npc.body_type) {
        return 0;
    }
    if npc.required == ArtLicense::AttributionOnly
        && license_of(item) != ArtLicense::AttributionOnly
    {
        return 0;
    }
    if item.has_tag("elderly_only") && !npc.elderly {
        return 0;
    }
    if item.has_tag("child") && npc.body_type != "child" {
        return 0;
    }
    if item.has_tag("masc") && !npc.presents_masc {
        return 0;
    }
    if item.has_tag("fem") && npc.presents_masc {
        return 0;
    }
    // A winter beard is for an old man, and a teenager is not one yet. The tag
    // means "belongs with an old face", so on a young base it means "not here".
    if item.has_tag("elderly") && npc.young() {
        return 0;
    }
    // A casting that asks for an elder, or for somebody who is plainly not one,
    // decides the head rather than hoping. Only heads are constrained: an old
    // face may still turn up under a young man's hat, and a grown adult with a
    // winter beard is a look rather than a contradiction.
    if item.type_name == "head" {
        if let Some(wanted) = npc.cast.elderly() {
            if item.has_tag("elderly") != wanted {
                return 0;
            }
        }
    }
    let favoured = npc.elderly && (item.has_tag("elderly") || item.has_tag("elderly_favoured"));
    item.weight * if favoured { ELDERLY_FAVOUR } else { 1 }
}

/// Pick from a slot without rolling its fill chance. Used for the head, which is
/// never optional and is needed before the rest of the traveller exists.
fn choose<'a>(rng: &mut Rng, wardrobe: &'a Wardrobe, slot: &str, npc: &Npc) -> Option<&'a Item> {
    let items = &wardrobe.slot(slot)?.items;
    rng.weighted(items, |item| eligibility(item, npc))
}

/// Roll a slot's fill chance and, if it lands, add a piece.
fn fill(rng: &mut Rng, npc: &mut Npc, wardrobe: &'static Wardrobe, slot: &str) -> bool {
    let Some(spec) = wardrobe.slot(slot) else {
        return false;
    };
    if !rng.odds(spec.chance) {
        return false;
    }
    let Some(item) = rng.weighted(&spec.items, |item| eligibility(item, npc)) else {
        return false;
    };
    push(rng, npc, item);
    true
}

/// Fill at most one of a set of slots that compete for the same part of a body.
/// Each slot keeps its own fill chance, so the group as a whole stays rare.
fn one_of<'s>(
    rng: &mut Rng,
    npc: &mut Npc,
    wardrobe: &'static Wardrobe,
    slots: &[&'s str],
) -> Option<&'s str> {
    slots
        .iter()
        .copied()
        .find(|slot| fill(rng, npc, wardrobe, slot))
}

/// Garments whose absence a viewer would notice, so a garment that cannot be
/// told apart from skin is worse than no garment at all — it reads as nudity
/// rather than as a plain shirt. A belt or a hat has no such problem.
const READS_AS_BARE_SKIN: [&str; 2] = ["clothes", "legs"];

/// How far apart two palettes have to be, as a distance between their average
/// colours, before a garment reads as cloth on a body rather than as the body.
///
/// Forty is where the eye stops being fooled at this sprite size. It costs the
/// darkest skin tones about a third of the muted palette — which sounds worse
/// than it is, because what it costs them is the browns and blacks they were
/// disappearing into.
const TELLS_APART: u32 = 40;

/// The average colour of a palette ramp.
fn ramp_average(ramp: &[String]) -> Option<[i32; 3]> {
    if ramp.is_empty() {
        return None;
    }
    let mut total = [0_i32; 3];
    for color in ramp {
        let parsed = rgb(color)?;
        for (sum, channel) in total.iter_mut().zip(parsed) {
            *sum += i32::from(channel);
        }
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let count = ramp.len() as i32;
    Some(total.map(|sum| sum / count))
}

/// Whether a garment in this colour would be visible on this skin.
///
/// Unknown palettes answer yes: refusing a colour because its ramp could not be
/// read would silently narrow the wardrobe, which is the worse failure.
fn tells_apart_from_skin(wardrobe: &Wardrobe, skin: &str, cloth: &str) -> bool {
    let Some(skin) = wardrobe.ramp("body", skin).and_then(ramp_average) else {
        return true;
    };
    let Some(cloth) = wardrobe.ramp("cloth", cloth).and_then(ramp_average) else {
        return true;
    };
    let distance: i32 = skin.iter().zip(cloth).map(|(a, b)| (a - b) * (a - b)).sum();
    #[allow(clippy::cast_possible_wrap)]
    let threshold = (TELLS_APART * TELLS_APART) as i32;
    distance >= threshold
}

fn push(rng: &mut Rng, npc: &mut Npc, item: &Item) {
    let color = match item.source {
        ColorSource::Skin => npc.skin.clone(),
        ColorSource::Hair => npc.hair_color.clone(),
        ColorSource::Fixed => rng
            .weighted(&item.options, |_| 1)
            .cloned()
            .unwrap_or_default(),
        ColorSource::Cloth => {
            let wardrobe = Wardrobe::bundled();
            let palette = match npc.era {
                Era::Scavenged => &wardrobe.palettes.cloth_muted,
                Era::Dyed => &wardrobe.palettes.cloth_bright,
            };
            // A piece that ships baked colours only offers some of the palette;
            // weighting the intersection keeps a brown-heavy cast brown-heavy
            // even when a garment happens not to come in walnut.
            let allowed: Vec<&wardrobe::Weighted> = if item.options.is_empty() {
                palette.iter().collect()
            } else {
                palette
                    .iter()
                    .filter(|c| item.options.contains(&c.color))
                    .collect()
            };
            // And a shirt has to look like a shirt. LPC shades a torso gently,
            // so a garment within a few tones of the wearer's skin does not
            // read as brown cloth on a brown body — it reads as somebody with
            // no shirt on. Narrow to colours that can be told apart, and fall
            // back to the full set rather than leave anyone undressed.
            let legible: Vec<&wardrobe::Weighted> =
                if READS_AS_BARE_SKIN.contains(&item.type_name.as_str()) {
                    allowed
                        .iter()
                        .copied()
                        .filter(|c| tells_apart_from_skin(wardrobe, &npc.skin, &c.color))
                        .collect()
                } else {
                    Vec::new()
                };
            let pool = if legible.is_empty() {
                &allowed
            } else {
                &legible
            };
            rng.weighted(pool, |c| c.weight)
                .map_or_else(String::new, |c| c.color.clone())
        }
    };

    let (variant, recolor) = match item.field {
        ColorField::Variant => (color, String::new()),
        ColorField::Recolor => (String::new(), color),
        ColorField::None => (String::new(), String::new()),
    };

    let mut name = item.name.clone();
    if !variant.is_empty() || !recolor.is_empty() {
        // The app's own format: `Name (variant)`, `Name (recolor)`, or both
        // separated by a bar. Matching it means a generated file and a
        // hand-made one are indistinguishable once loaded.
        let joiner = if variant.is_empty() || recolor.is_empty() {
            ""
        } else {
            " | "
        };
        let _ = write!(name, " ({variant}{joiner}{recolor})");
    }

    npc.pieces.push(Piece {
        slot: item.type_name.clone(),
        item_id: item.id.clone(),
        base_name: item.name.clone(),
        name,
        variant,
        recolor,
        license: license_of(item),
    });
}

/// Where the LPC app lives. Its hash carries a whole character, so a generated
/// traveller can be opened, inspected and edited from a link.
const GENERATOR_URL: &str =
    "https://liberatedpixelcup.github.io/Universal-LPC-Spritesheet-Character-Generator/";

/// Percent-encode a hash value the way the app's own `encodeURIComponent` does.
///
/// Most piece names survive untouched, but a few carry a slash — "Side Parted
/// w/Bangs" is a real hair style — and a raw slash in a hash makes the app read
/// a different piece, or none.
fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            // The unreserved set `encodeURIComponent` leaves alone.
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(byte as char),
            _ => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

impl Npc {
    /// A link that opens this traveller in the LPC web generator.
    ///
    /// The hash format is the app's own: `sex` for the body base (named that
    /// way for backwards compatibility with old links), then one parameter per
    /// slot valued `Piece_Name_colour`.
    #[must_use]
    pub fn generator_url(&self) -> String {
        let mut url = format!("{GENERATOR_URL}#sex={}", self.body_type);
        for piece in &self.pieces {
            let name = piece.base_name.replace(' ', "_");
            let joiner = if piece.variant.is_empty() || piece.recolor.is_empty() {
                ""
            } else {
                "|"
            };
            let color = format!("{}{joiner}{}", piece.variant, piece.recolor);
            let separator = if color.is_empty() { "" } else { "_" };
            let value = escape(&format!("{name}{separator}{color}"));
            let _ = write!(url, "&{}={value}", piece.slot);
        }
        url
    }

    /// This traveller as the LPC app's `character.json`.
    ///
    /// Only the fields the app's importer reads are written. `layers` and
    /// `credits` are export-only extras the app recomputes from the selections
    /// on load, and writing them here would mean duplicating its sprite-path
    /// and licence resolution for no gain.
    #[must_use]
    pub fn to_ulpc_json(&self) -> String {
        let selections: serde_json::Map<String, serde_json::Value> = self
            .pieces
            .iter()
            .map(|piece| {
                (
                    piece.slot.clone(),
                    serde_json::json!({
                        "itemId": piece.item_id,
                        "subId": serde_json::Value::Null,
                        "variant": piece.variant,
                        "recolor": piece.recolor,
                        "name": piece.name,
                    }),
                )
            })
            .collect();

        let document = serde_json::json!({
            "version": 2,
            "bodyType": self.body_type,
            "selections": selections,
            "selectedAnimation": "walk",
            "showTransparencyGrid": true,
            "applyTransparencyMask": false,
            // Heads, faces, noses and wrinkles carry no colour of their own;
            // the app tints them from whichever selection is the body. Turning
            // this off would leave a traveller with a mismatched face.
            "matchBodyColorEnabled": true,
            "compactDisplay": false,
            "url": self.generator_url(),
        });
        serde_json::to_string_pretty(&document).unwrap_or_default()
    }

    /// One line for scanning a generated cast: who this is, in the order the
    /// pieces are drawn.
    #[must_use]
    pub fn describe(&self) -> String {
        let mut line = format!(
            "{}{} skin {}",
            self.body_type,
            if self.elderly { ", elderly" } else { "" },
            self.skin
        );
        for piece in &self.pieces {
            if piece.slot == "body" {
                continue;
            }
            let color = piece.color();
            let _ = if color.is_empty() {
                write!(line, ", {}", piece.item_id)
            } else {
                write!(line, ", {} {color}", piece.item_id)
            };
        }
        line
    }

    /// The piece filling a slot, if any.
    #[must_use]
    pub fn piece(&self, slot: &str) -> Option<&Piece> {
        self.pieces.iter().find(|piece| piece.slot == slot)
    }
}

impl Npc {
    /// A traveller assembled from named pieces instead of rolled from a seed.
    ///
    /// Every piece is `(slot, item id, colour)`, in the order it should be
    /// drawn when two of them claim the same `zPos`. Unknown ids are refused
    /// rather than skipped: a character described in terms the wardrobe no
    /// longer knows is a character who would quietly lose a limb.
    ///
    /// This is how a fixed cast — a saved character, or the reference sheets
    /// the compositor is tested against — becomes something [`draws`] can
    /// answer for.
    ///
    /// [`draws`]: Npc::draws
    pub fn assembled(
        body_type: &str,
        chosen: &[(impl AsRef<str>, impl AsRef<str>, impl AsRef<str>)],
    ) -> Result<Self, String> {
        let wardrobe = Wardrobe::bundled();
        let mut pieces = Vec::with_capacity(chosen.len());
        for (slot, id, color) in chosen {
            let (slot, id, color) = (slot.as_ref(), id.as_ref(), color.as_ref());
            let item = wardrobe
                .item(slot, id)
                .ok_or_else(|| format!("{slot}: the wardrobe has no piece called {id}"))?;
            if !item.fits(body_type) {
                return Err(format!("{id} has no art for a {body_type} body"));
            }
            let (variant, recolor) = match item.field {
                ColorField::Variant => (color.to_owned(), String::new()),
                ColorField::Recolor => (String::new(), color.to_owned()),
                ColorField::None => (String::new(), String::new()),
            };
            pieces.push(Piece {
                slot: item.type_name.clone(),
                item_id: item.id.clone(),
                base_name: item.name.clone(),
                name: item.name.clone(),
                variant,
                recolor,
                license: license_of(item),
            });
        }
        Ok(Self {
            body_type: body_type.to_owned(),
            skin: String::new(),
            hair_color: String::new(),
            presents_masc: false,
            elderly: false,
            era: Era::default(),
            required: ArtLicense::ShareAlike,
            cast: Cast::Anyone,
            seed: 0,
            pieces,
        })
    }
}

/// The one animation the game draws visitors from.
///
/// They walk in, they stand on frame 0 of the south-facing row, they walk out.
/// Nothing else in a 54-row LPC action sheet ever reaches a screen, so nothing
/// else is copied into the runtime tree.
pub const ANIMATION: &str = "walk";

/// One sprite sheet to draw, and what to do to its colours on the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draw {
    /// Path relative to the root `scripts/build-npc-art.py` writes, e.g.
    /// `hair/plain/adult/walk.png`.
    pub sheet: String,
    /// LPC's stacking number, kept for the caller's benefit; [`Npc::draws`]
    /// has already sorted by it.
    pub z: i32,
    /// Colours to replace, as `(from, to)` RGB pairs. Empty for art that ships
    /// in the colour it is worn in.
    pub swap: Vec<([u8; 3], [u8; 3])>,
}

impl Npc {
    /// Every sheet this traveller is drawn from, bottom layer first.
    ///
    /// This is the whole contract with whatever does the compositing: stack
    /// these in order, swapping the colours each one names. Pieces sharing a
    /// `zPos` keep the order they were chosen in, which is what the LPC web app
    /// does with the insertion order of its own selections — matching it is why
    /// a sheet composited here and one exported from the app come out
    /// byte-for-byte identical.
    ///
    /// A layer with no art for this body type, or whose `${head}` path has no
    /// substitution for the head that was rolled, is skipped. Both are ordinary
    /// gaps in the catalogue rather than errors: the app draws nothing there
    /// too.
    #[must_use]
    pub fn draws(&self) -> Vec<Draw> {
        let wardrobe = Wardrobe::bundled();
        let mut found: Vec<(i32, usize, Draw)> = Vec::new();
        for (order, piece) in self.pieces.iter().enumerate() {
            let Some(item) = wardrobe.item(&piece.slot, &piece.item_id) else {
                continue;
            };
            let swap = color_swap(wardrobe, item, piece.color());
            for layer in &item.layers {
                let Some(path) = layer.paths.get(&self.body_type) else {
                    continue;
                };
                let Some(path) = self.resolve_path(item, path) else {
                    continue;
                };
                let sheet = if item.field == ColorField::Variant {
                    format!("{path}{ANIMATION}/{}.png", piece.color().replace(' ', "_"))
                } else {
                    format!("{path}{ANIMATION}.png")
                };
                found.push((
                    layer.z,
                    order,
                    Draw {
                        sheet,
                        z: layer.z,
                        swap: swap.clone(),
                    },
                ));
            }
        }
        found.sort_by_key(|(z, order, _)| (*z, *order));
        found.into_iter().map(|(_, _, draw)| draw).collect()
    }

    /// Fill in a `${type}` placeholder from whatever fills that slot.
    ///
    /// Faces live under a directory named for the head above them, so an
    /// expression's path is not real until the head is known.
    fn resolve_path(&self, item: &Item, path: &str) -> Option<String> {
        let mut path = path.to_owned();
        while let Some(start) = path.find("${") {
            let end = start + path[start..].find('}')?;
            let key = path[start + 2..end].to_owned();
            let chosen = self.piece(&key)?;
            let value = item
                .replace
                .get(&key)?
                .get(&chosen.base_name.replace(' ', "_"))?;
            path = format!("{}{value}{}", &path[..start], &path[end + 1..]);
        }
        Some(path)
    }
}

/// The palette swap a piece needs, once a colour has been rolled for it.
fn color_swap(wardrobe: &Wardrobe, item: &Item, color: &str) -> Vec<([u8; 3], [u8; 3])> {
    let Some(recolor) = &item.recolor else {
        return Vec::new();
    };
    let Some(target) = wardrobe.ramp(&recolor.material, color) else {
        return Vec::new();
    };
    recolor
        .from
        .iter()
        .zip(target)
        .filter_map(|(from, to)| Some((rgb(from)?, rgb(to)?)))
        .collect()
}

/// A `#rrggbb` from the palette files. LPC spells them in both cases.
fn rgb(value: &str) -> Option<[u8; 3]> {
    let digits = value.strip_prefix('#').unwrap_or(value);
    if digits.len() != 6 {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cast(count: u64, era: Era) -> Vec<Npc> {
        (1..=count)
            .map(|seed| generate(seed.wrapping_mul(SEED_MIX), era))
            .collect()
    }

    /// A cast fit to appear in a screenshot. Uses the brighter era on purpose:
    /// the licence bar and the palette are independent, and this is where they
    /// would collide if they were not.
    fn safe_cast(count: u64) -> Vec<Npc> {
        (1..=count)
            .map(|seed| {
                generate_for(
                    seed.wrapping_mul(SEED_MIX),
                    Era::Dyed,
                    ArtLicense::AttributionOnly,
                )
            })
            .collect()
    }

    #[test]
    fn a_seed_always_makes_the_same_person() {
        let once = generate(12_345, Era::Scavenged);
        let twice = generate(12_345, Era::Scavenged);
        assert_eq!(once.pieces, twice.pieces);
        assert_eq!(once.body_type, twice.body_type);
    }

    #[test]
    fn different_seeds_make_different_people() {
        let people: std::collections::HashSet<String> = cast(200, Era::Scavenged)
            .iter()
            .map(Npc::describe)
            .collect();
        assert!(
            people.len() > 190,
            "only {} distinct travellers in 200",
            people.len()
        );
    }

    /// Sampling finds a naked traveller eventually; this finds the cause
    /// directly. LPC has body bases with almost no wardrobe behind them — the
    /// muscular base owns a pair of suspenders and nothing else — and adding one
    /// to the body-type table should fail here rather than in a screenshot.
    #[test]
    fn every_body_base_has_something_to_wear() {
        let wardrobe = Wardrobe::bundled();
        for base in &wardrobe.body_types {
            for slot in ["body", "head", "clothes", "legs"] {
                let items = &wardrobe.slot(slot).expect("slot exists").items;
                assert!(
                    items.iter().any(|item| item.fits(&base.id)),
                    "no {slot} is drawn for the {} base",
                    base.id
                );
            }
        }
    }

    #[test]
    fn everyone_is_dressed() {
        for npc in cast(500, Era::Scavenged) {
            assert!(npc.piece("body").is_some(), "no body: {}", npc.describe());
            assert!(npc.piece("head").is_some(), "no head: {}", npc.describe());
            assert!(
                npc.piece("clothes").is_some(),
                "no shirt: {}",
                npc.describe()
            );
            assert!(
                npc.piece("legs").is_some(),
                "no trousers: {}",
                npc.describe()
            );
        }
    }

    /// The whole point of tracking `bodies` per piece: a selection the base has
    /// no art for is silently invisible in the app, which is worse than an
    /// error because it looks like a drawing bug.
    #[test]
    fn nothing_is_worn_by_a_body_that_cannot_wear_it() {
        let wardrobe = Wardrobe::bundled();
        for npc in cast(500, Era::Dyed) {
            for piece in &npc.pieces {
                let item = wardrobe
                    .slot(&piece.slot)
                    .and_then(|slot| slot.items.iter().find(|i| i.id == piece.item_id))
                    .expect("piece came from the wardrobe");
                assert!(
                    item.fits(&npc.body_type),
                    "{} on a {} body: {}",
                    piece.item_id,
                    npc.body_type,
                    npc.describe()
                );
            }
        }
    }

    /// Waystation is not open source, so GPL is not a licence it can pick. The
    /// wardrobe is generated with that bar enforced; this is the tripwire for a
    /// hand-edited data file, which would otherwise ship quietly.
    #[test]
    fn no_piece_of_art_is_gpl_only() {
        let wardrobe = Wardrobe::bundled();
        assert_eq!(wardrobe.license_policy, "permissive");
        for (name, slot) in &wardrobe.slots {
            for item in &slot.items {
                assert!(
                    !item.licenses.is_empty(),
                    "{} ({name}) records no usable licence",
                    item.id
                );
                assert!(
                    !item.licenses.iter().any(|l| l.starts_with("GPL")),
                    "{} ({name}) is only usable under GPL",
                    item.id
                );
            }
        }
    }

    /// A traveller destined for a screenshot must carry no share-alike art at
    /// all, because the flat image would also contain purchased tilesets that
    /// cannot be relicensed to match.
    #[test]
    fn a_screenshot_safe_traveller_carries_nothing_share_alike() {
        for npc in safe_cast(600) {
            assert_eq!(
                npc.art_license(),
                ArtLicense::AttributionOnly,
                "encumbered by {:?}: {}",
                npc.encumbered_pieces()
                    .iter()
                    .map(|p| &p.item_id)
                    .collect::<Vec<_>>(),
                npc.describe()
            );
            assert!(npc.encumbered_pieces().is_empty());
        }
    }

    /// The strict bar removes whole slots, so this is the guard that it does
    /// not also remove something a traveller cannot go without.
    #[test]
    fn a_screenshot_safe_traveller_is_still_dressed() {
        for npc in safe_cast(600) {
            for slot in ["body", "head", "clothes", "legs"] {
                assert!(npc.piece(slot).is_some(), "no {slot}: {}", npc.describe());
            }
        }
    }

    /// Every base has to survive the strict bar, not just the common ones.
    #[test]
    fn every_body_base_can_be_dressed_from_unencumbered_art_alone() {
        let wardrobe = Wardrobe::bundled();
        for base in &wardrobe.body_types {
            for slot in ["body", "head", "clothes", "legs"] {
                let items = &wardrobe.slot(slot).expect("slot exists").items;
                assert!(
                    items.iter().any(|item| item.fits(&base.id)
                        && license_of(item) == ArtLicense::AttributionOnly),
                    "the {} base has no unencumbered {slot}",
                    base.id
                );
            }
        }
    }

    /// The default bar is the absence of a restriction, not a request for
    /// share-alike art — plenty of travellers come out clean on their own.
    #[test]
    fn the_default_bar_still_produces_unencumbered_travellers() {
        let clean = cast(400, Era::Scavenged)
            .iter()
            .filter(|npc| npc.art_license() == ArtLicense::AttributionOnly)
            .count();
        assert!(clean > 40, "only {clean} of 400 came out unencumbered");
    }

    #[test]
    fn a_sheet_is_share_alike_if_any_one_piece_is() {
        let npc = cast(400, Era::Scavenged)
            .into_iter()
            .find(|npc| npc.art_license() == ArtLicense::ShareAlike)
            .expect("some traveller picks up share-alike art");
        assert!(!npc.encumbered_pieces().is_empty());
        for piece in npc.encumbered_pieces() {
            assert_eq!(piece.license, ArtLicense::ShareAlike);
        }
    }

    #[test]
    fn one_slot_holds_one_piece() {
        for npc in cast(300, Era::Scavenged) {
            let mut seen = std::collections::HashSet::new();
            for piece in &npc.pieces {
                assert!(seen.insert(piece.slot.clone()), "two {} pieces", piece.slot);
            }
        }
    }

    #[test]
    fn nobody_wears_two_things_on_their_head_or_two_at_the_waist() {
        for npc in cast(500, Era::Scavenged) {
            let head = ["hat", "headcover", "bandana"]
                .iter()
                .filter(|s| npc.piece(s).is_some());
            assert!(head.count() <= 1, "layered headgear: {}", npc.describe());
            let waist = ["belt", "sash"].iter().filter(|s| npc.piece(s).is_some());
            assert!(waist.count() <= 1, "two belts: {}", npc.describe());
            assert!(
                !(npc.piece("overalls").is_some() && npc.piece("belt").is_some()),
                "belt under suspenders: {}",
                npc.describe()
            );
            assert!(
                !(npc.piece("apron").is_some() && npc.piece("overalls").is_some()),
                "apron over overalls: {}",
                npc.describe()
            );
        }
    }

    #[test]
    fn a_hood_is_worn_instead_of_hair_not_over_it() {
        for npc in cast(800, Era::Scavenged) {
            if npc.piece("hat").is_some() {
                assert!(
                    npc.piece("hair").is_none(),
                    "hair through a hood: {}",
                    npc.describe()
                );
            }
        }
    }

    /// White hair on a twenty-year-old reads as a costume, and ULPC's `ash` is
    /// purple rather than the ash blonde its name promises.
    #[test]
    fn hair_colours_stay_believable() {
        for npc in cast(600, Era::Scavenged) {
            assert_ne!(npc.hair_color, "ash", "purple hair: {}", npc.describe());
            if !npc.elderly {
                for old in ["gray", "white", "platinum"] {
                    assert_ne!(npc.hair_color, old, "young and {old}: {}", npc.describe());
                }
            }
        }
    }

    #[test]
    fn only_old_faces_get_old_details() {
        for npc in cast(500, Era::Scavenged) {
            if !npc.elderly {
                assert!(
                    npc.piece("wrinkles").is_none(),
                    "young and lined: {}",
                    npc.describe()
                );
            }
        }
    }

    #[test]
    fn children_stay_children() {
        let wardrobe = Wardrobe::bundled();
        for npc in cast(800, Era::Scavenged) {
            if npc.body_type != "child" {
                continue;
            }
            assert!(
                npc.piece("beard").is_none(),
                "bearded child: {}",
                npc.describe()
            );
            assert!(
                npc.piece("weapon").is_none(),
                "armed child: {}",
                npc.describe()
            );
            let head = npc.piece("head").expect("child has a head");
            assert_eq!(head.item_id, "heads_human_child");
            // And nothing meant for a grown body sneaks in.
            for piece in &npc.pieces {
                let item = wardrobe
                    .slot(&piece.slot)
                    .and_then(|slot| slot.items.iter().find(|i| i.id == piece.item_id))
                    .expect("piece came from the wardrobe");
                assert!(
                    item.fits("child"),
                    "{} on a child: {}",
                    piece.item_id,
                    npc.describe()
                );
            }
        }
    }

    #[test]
    fn the_muted_era_stays_muted() {
        let bright = [
            "red", "yellow", "teal", "blue", "navy", "purple", "lavender", "sky", "pink",
        ];
        for npc in cast(500, Era::Scavenged) {
            for piece in &npc.pieces {
                assert!(
                    !bright.contains(&piece.color()),
                    "{} in {} before the dyes: {}",
                    piece.item_id,
                    piece.color(),
                    npc.describe()
                );
            }
        }
    }

    /// The failure this catches looks like a bug in the art: a traveller walks
    /// into the court apparently naked, because their shirt is the same brown
    /// as they are and LPC shades a torso too gently to say otherwise.
    #[test]
    fn nobody_wears_a_shirt_the_colour_of_their_own_skin() {
        let wardrobe = Wardrobe::bundled();
        for npc in cast(600, Era::Scavenged)
            .into_iter()
            .chain(cast(600, Era::Dyed))
        {
            for slot in READS_AS_BARE_SKIN {
                let Some(piece) = npc.piece(slot) else {
                    continue;
                };
                assert!(
                    tells_apart_from_skin(wardrobe, &npc.skin, piece.color()),
                    "{} in {} on {} skin is invisible: {}",
                    piece.item_id,
                    piece.color(),
                    npc.skin,
                    npc.describe()
                );
            }
        }
    }

    /// The rule narrows the palette, and it must not narrow it to nothing —
    /// every skin tone has to keep a wardrobe worth having.
    #[test]
    fn every_skin_tone_keeps_plenty_of_colours_to_wear() {
        let wardrobe = Wardrobe::bundled();
        for skin in &wardrobe.palettes.skin {
            for (era, palette) in [
                ("scavenged", &wardrobe.palettes.cloth_muted),
                ("dyed", &wardrobe.palettes.cloth_bright),
            ] {
                let usable = palette
                    .iter()
                    .filter(|cloth| tells_apart_from_skin(wardrobe, &skin.color, &cloth.color))
                    .count();
                assert!(
                    usable * 2 >= palette.len(),
                    "{} skin can wear only {usable} of {} {era} colours",
                    skin.color,
                    palette.len()
                );
            }
        }
    }

    #[test]
    fn the_face_is_the_colour_of_the_body() {
        for npc in cast(300, Era::Scavenged) {
            for slot in ["head", "expression", "nose", "wrinkles"] {
                if let Some(piece) = npc.piece(slot) {
                    assert_eq!(
                        piece.color(),
                        npc.skin,
                        "{slot} mismatched: {}",
                        npc.describe()
                    );
                }
            }
        }
    }

    #[test]
    fn eyebrows_match_the_hair() {
        for npc in cast(300, Era::Scavenged) {
            if let Some(brows) = npc.piece("eyebrows") {
                assert_eq!(brows.color(), npc.hair_color, "brows: {}", npc.describe());
            }
        }
    }

    #[test]
    fn the_export_is_shaped_like_the_apps_own() {
        let npc = generate(99, Era::Scavenged);
        let document: serde_json::Value =
            serde_json::from_str(&npc.to_ulpc_json()).expect("valid JSON");
        assert_eq!(document["version"], 2);
        assert_eq!(document["bodyType"], npc.body_type.as_str());
        assert_eq!(document["matchBodyColorEnabled"], true);
        let body = &document["selections"]["body"];
        assert_eq!(body["itemId"], "body");
        assert_eq!(body["recolor"], npc.skin.as_str());
        assert_eq!(body["name"], format!("Body Color ({})", npc.skin).as_str());
        assert!(body["subId"].is_null());
        assert!(document["url"]
            .as_str()
            .is_some_and(|u| u.contains("#sex=")));
    }

    #[test]
    fn the_link_names_every_piece() {
        let npc = generate(7, Era::Dyed);
        let url = npc.generator_url();
        for piece in &npc.pieces {
            let expected = format!(
                "&{}={}",
                piece.slot,
                escape(&piece.base_name.replace(' ', "_"))
            );
            assert!(url.contains(&expected), "{expected} missing from {url}");
        }
    }

    /// A raw slash in the hash sends the app looking for a piece that is not
    /// there, and "Side Parted w/Bangs" is a real hair style.
    #[test]
    fn a_slash_in_a_name_survives_the_link() {
        assert_eq!(
            escape("Side_Parted_w/Bangs_2_gray"),
            "Side_Parted_w%2FBangs_2_gray"
        );
        assert_eq!(escape("5_O'clock_Shadow_black"), "5_O'clock_Shadow_black");
        for npc in cast(400, Era::Dyed) {
            let url = npc.generator_url();
            assert!(
                !url[url.find('#').expect("a hash")..].contains('/'),
                "{url}"
            );
        }
    }

    #[test]
    fn nobody_young_is_given_an_old_face() {
        for npc in cast(600, Era::Scavenged) {
            if npc.body_type == "teen" || npc.body_type == "child" {
                assert!(!npc.elderly, "young and elderly: {}", npc.describe());
                assert!(
                    npc.piece("wrinkles").is_none(),
                    "lined teen: {}",
                    npc.describe()
                );
                let beard = npc.piece("beard").map(|p| p.item_id.as_str());
                assert_ne!(
                    beard,
                    Some("beards_winter"),
                    "teen patriarch: {}",
                    npc.describe()
                );
            }
        }
    }

    /// The `teen` base is one silhouette that has to carry every kind of young
    /// person, so the head above it is rolled rather than implied.
    #[test]
    fn teenagers_come_both_ways() {
        let teens: Vec<Npc> = cast(600, Era::Scavenged)
            .into_iter()
            .filter(|n| n.body_type == "teen")
            .collect();
        assert!(teens.iter().any(|n| n.presents_masc), "no masc teens");
        assert!(teens.iter().any(|n| !n.presents_masc), "no fem teens");
    }

    fn cast_of(count: u64, cast: Cast) -> Vec<Npc> {
        (1..=count)
            .map(|seed| {
                generate_with(
                    seed.wrapping_mul(SEED_MIX),
                    Casting {
                        cast,
                        ..Casting::default()
                    },
                )
            })
            .collect()
    }

    /// The game's arrivals are authored shapes — someone alone, two siblings,
    /// an old hand — and the shape only works if the people standing in it are
    /// the right age. "Two of them, and one of those is small" has to be true.
    #[test]
    fn a_casting_gets_the_age_it_asked_for() {
        for npc in cast_of(200, Cast::Child) {
            assert_eq!(npc.body_type, "child", "{}", npc.describe());
        }
        for npc in cast_of(200, Cast::Youth) {
            assert_eq!(npc.body_type, "teen", "{}", npc.describe());
        }
        for npc in cast_of(200, Cast::Grown) {
            assert!(
                matches!(npc.body_type.as_str(), "male" | "female"),
                "{}",
                npc.describe()
            );
            assert!(!npc.elderly, "asked for grown, got old: {}", npc.describe());
        }
        for npc in cast_of(200, Cast::Elder) {
            assert!(
                matches!(npc.body_type.as_str(), "male" | "female"),
                "{}",
                npc.describe()
            );
            assert!(npc.elderly, "asked for old, got young: {}", npc.describe());
        }
    }

    /// A constraint that quietly emptied a slot would leave somebody standing
    /// in the court in their skin.
    #[test]
    fn a_cast_traveller_is_still_dressed_and_still_varied() {
        for cast in [Cast::Grown, Cast::Elder, Cast::Youth, Cast::Child] {
            let people = cast_of(120, cast);
            for npc in &people {
                for slot in ["body", "head", "clothes", "legs"] {
                    assert!(
                        npc.piece(slot).is_some(),
                        "no {slot} for {cast:?}: {}",
                        npc.describe()
                    );
                }
            }
            let distinct: std::collections::HashSet<String> =
                people.iter().map(Npc::describe).collect();
            assert!(
                distinct.len() > 100,
                "only {} distinct {cast:?} travellers in 120",
                distinct.len()
            );
        }
    }

    /// Both genders have to survive every casting; a constraint that only
    /// admitted one would be invisible until someone noticed every elder in the
    /// game was a man.
    #[test]
    fn every_casting_comes_both_ways() {
        for cast in [Cast::Grown, Cast::Elder, Cast::Youth, Cast::Child] {
            let people = cast_of(200, cast);
            assert!(
                people.iter().any(|npc| npc.presents_masc),
                "no masc {cast:?}"
            );
            assert!(
                people.iter().any(|npc| !npc.presents_masc),
                "no fem {cast:?}"
            );
        }
    }

    #[test]
    fn draws_come_out_in_stacking_order_with_the_body_underneath() {
        for npc in cast(200, Era::Scavenged) {
            let draws = npc.draws();
            assert!(!draws.is_empty(), "nothing to draw: {}", npc.describe());
            assert!(
                draws.windows(2).all(|pair| pair[0].z <= pair[1].z),
                "out of order: {}",
                npc.describe()
            );
            let body = draws
                .iter()
                .position(|draw| draw.sheet.starts_with("body/"))
                .expect("everyone has a body");
            let clothes = draws
                .iter()
                .position(|draw| draw.sheet.starts_with("torso/"))
                .expect("everyone has a shirt");
            assert!(body < clothes, "dressed underneath: {}", npc.describe());
        }
    }

    /// A palette-recoloured piece that came back with an empty swap would draw
    /// in whatever the artist happened to use — orange hair on everybody.
    #[test]
    fn every_recoloured_piece_gets_a_full_swap() {
        let wardrobe = Wardrobe::bundled();
        for npc in cast(200, Era::Dyed) {
            for piece in &npc.pieces {
                let item = wardrobe
                    .item(&piece.slot, &piece.item_id)
                    .expect("piece came from the wardrobe");
                let Some(recolor) = &item.recolor else {
                    continue;
                };
                let target = wardrobe.ramp(&recolor.material, piece.color());
                assert_eq!(
                    target.map(<[String]>::len),
                    Some(recolor.from.len()),
                    "{} is drawn in a {} ramp of {} colours, and the wardrobe has no \
                     matching '{}' to swap it to",
                    piece.item_id,
                    recolor.material,
                    recolor.from.len(),
                    piece.color()
                );
                assert_eq!(
                    color_swap(wardrobe, item, piece.color()).len(),
                    recolor.from.len(),
                    "{}: the swap came back short, so some of its colours would \
                     survive the recolour",
                    piece.item_id
                );
            }
            // And nothing draws with a half-applied palette.
            for draw in npc.draws() {
                assert!(
                    draw.swap.is_empty() || draw.swap.len() >= 6,
                    "{}: {} colours is not a whole LPC ramp",
                    draw.sheet,
                    draw.swap.len()
                );
            }
        }
    }

    /// The wardrobe names sprite files; this is the check that they are on
    /// disk. Nothing else catches a piece whose art was never copied — the
    /// layer simply does not draw, and the traveller loses a garment.
    #[test]
    fn every_sheet_a_traveller_needs_is_in_the_runtime_tree() {
        let art = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-assets/npc");
        if !art.is_dir() {
            eprintln!(
                "no {} — generated visitor art was not built, so the sheets \
                 travellers name are unchecked. Run `make assets` with an LPC checkout.",
                art.display()
            );
            return;
        }
        let mut checked = 0;
        for npc in cast(400, Era::Dyed) {
            for draw in npc.draws() {
                assert!(
                    art.join(&draw.sheet).is_file(),
                    "{} needs {}, which is not in the runtime tree",
                    npc.describe(),
                    draw.sheet
                );
                checked += 1;
            }
        }
        assert!(checked > 400, "only {checked} sheets checked");
    }

    /// A named cast has to survive a wardrobe that has moved on. Silently
    /// skipping an id the wardrobe no longer knows would draw a person with a
    /// missing limb and no complaint.
    #[test]
    fn an_assembled_traveller_refuses_pieces_the_wardrobe_does_not_have() {
        let good = Npc::assembled(
            "male",
            &[
                ("body", "body", "light"),
                ("head", "heads_human_male", "light"),
            ],
        )
        .expect("both pieces are real");
        assert_eq!(good.pieces.len(), 2);
        assert_eq!(good.body_type, "male");
        assert!(!good.draws().is_empty());

        assert!(Npc::assembled("male", &[("hair", "hair_of_flame", "black")]).is_err());
        // Real piece, wrong body: the child head has no adult art.
        assert!(Npc::assembled("male", &[("head", "heads_human_child", "light")]).is_err());
    }

    /// The one carried item, and it is a walking aid.
    #[test]
    fn the_only_thing_anyone_carries_is_a_cane() {
        for npc in cast(1000, Era::Dyed) {
            if let Some(weapon) = npc.piece("weapon") {
                assert_eq!(weapon.item_id, "weapon_polearm_cane");
            }
        }
    }
}
