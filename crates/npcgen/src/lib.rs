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
//! What this crate does *not* do is draw anything. It names pieces; compositing
//! them into a sheet is a separate job, and until that exists the pieces are
//! turned into art through the web tool.

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
    let wardrobe = Wardrobe::bundled();
    let mut rng = Rng::new(seed);

    let body_type = rng
        .weighted(&wardrobe.body_types, |b| b.weight)
        .map_or_else(|| "male".to_owned(), |b| b.id.clone());

    let skin = pick_color(&mut rng, &wardrobe.palettes.skin);
    let presents_masc = presents_masc(&mut rng, &body_type);

    let mut npc = Npc {
        skin,
        hair_color: String::new(),
        presents_masc,
        elderly: false,
        era,
        required,
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
            rng.weighted(&allowed, |c| c.weight)
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
