//! The curated set of LPC pieces a Waystation traveller can be built from.
//!
//! The data itself lives in `data/wardrobe.json`, compiled in rather than
//! loaded from disk so the generator works identically in the browser build and
//! needs nothing from the asset server. It is written by
//! `scripts/build-npc-wardrobe.py`, which reads the Universal LPC Spritesheet
//! Generator's own sheet definitions; nothing here should be hand-edited.

use std::sync::OnceLock;

use serde::Deserialize;

const WARDROBE_JSON: &str = include_str!("../data/wardrobe.json");

/// Where a piece takes its colour from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorSource {
    /// The era's cloth palette, narrowed to what this piece offers.
    Cloth,
    /// Whatever hair colour the character already has.
    Hair,
    /// Whatever skin tone the character already has.
    Skin,
    /// The piece's own vocabulary, which is not a palette at all — a basket is
    /// "round" or "square" and a cane is only ever a cane.
    Fixed,
}

/// Which field of the exported selection carries the colour.
///
/// The LPC catalogue has two generations of asset. The older kind ships one
/// baked PNG per colour and names it in `variant`; the newer kind ships a
/// single sheet that the app recolours against a named palette, and names the
/// palette entry in `recolor`. The exported character JSON keeps both fields
/// and leaves the unused one empty, so the distinction has to survive here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorField {
    Variant,
    Recolor,
    None,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    /// The LPC `type_name`, which is also the key this piece occupies in the
    /// exported `selections` map. Two pieces sharing a type are mutually
    /// exclusive by construction.
    #[serde(rename = "type")]
    pub type_name: String,
    /// Body types this piece has art for. Selecting a piece outside this list
    /// is legal in the app and draws absolutely nothing, which is how you end
    /// up with a barefoot child who was supposed to have boots.
    pub bodies: Vec<String>,
    pub field: ColorField,
    pub source: ColorSource,
    pub weight: u32,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Colours this piece ships, for `variant` pieces. Palette-recoloured
    /// pieces accept the whole palette and leave this empty.
    #[serde(default)]
    pub options: Vec<String>,
    /// Licence families this piece's art may be used under here, in family
    /// form: `CC0`, `CC-BY`, `CC-BY-SA`, `OGA-BY`. LPC offers most art under
    /// several licences at once and GPL is usually one of them; the builder
    /// records only the ones this project can actually pick, so `GPL` never
    /// appears and an empty list means something went wrong.
    #[serde(default)]
    pub licenses: Vec<String>,
}

impl Item {
    #[must_use]
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    #[must_use]
    pub fn fits(&self, body_type: &str) -> bool {
        self.bodies.iter().any(|b| b == body_type)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Slot {
    /// How often the slot is filled at all. Required slots are 1.0.
    pub chance: f32,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Weighted {
    pub color: String,
    pub weight: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BodyType {
    pub id: String,
    pub weight: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Palettes {
    pub skin: Vec<Weighted>,
    pub hair: Vec<Weighted>,
    /// Drawn instead of `hair` once a head reads as elderly.
    pub hair_old: Vec<Weighted>,
    pub cloth_muted: Vec<Weighted>,
    pub cloth_bright: Vec<Weighted>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Wardrobe {
    pub lpc_revision: String,
    /// Which licence bar the art was filtered against: `permissive` excludes
    /// GPL-only art, `attribution` also excludes share-alike.
    pub license_policy: String,
    pub body_types: Vec<BodyType>,
    pub palettes: Palettes,
    pub slots: std::collections::BTreeMap<String, Slot>,
}

impl Wardrobe {
    /// The compiled-in wardrobe, parsed once.
    ///
    /// # Panics
    /// If the bundled JSON does not match the types above, which can only
    /// happen if someone changes the builder script and not this file.
    pub fn bundled() -> &'static Self {
        static CACHE: OnceLock<Wardrobe> = OnceLock::new();
        CACHE.get_or_init(|| {
            serde_json::from_str(WARDROBE_JSON).expect("bundled wardrobe.json is malformed")
        })
    }

    #[must_use]
    pub fn slot(&self, name: &str) -> Option<&Slot> {
        self.slots.get(name)
    }
}
