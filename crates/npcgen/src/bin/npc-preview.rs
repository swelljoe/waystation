//! Generate a cast of travellers and lay them out for inspection.
//!
//! Reviewing procedural people means looking at them, and the fastest way to do
//! that is the LPC web generator itself: every traveller written here comes with
//! a link that opens them in the tool, fully dressed. The contact sheet in
//! `scripts/preview-npcs.py` renders the same folder without leaving the
//! terminal.
//!
//!     cargo run -p waystation-npcgen --bin npc-preview -- --count 24
//!     cargo run -p waystation-npcgen --bin npc-preview -- --era dyed --seed 99

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use waystation_npcgen::{generate_for, ArtLicense, Era, Npc};

const DEFAULT_OUT: &str = "target/npc-preview";

struct Args {
    count: u64,
    seed: u64,
    era: Era,
    art: ArtLicense,
    out: PathBuf,
}

const fn usage() -> &'static str {
    "usage: npc-preview [--count N] [--seed N] [--era scavenged|dyed] \
[--art any|attribution-only] [--out DIR]"
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        count: 24,
        // Any odd constant will do. Fixed rather than clock-derived so a review
        // pass can be repeated and a rogue traveller can be reported by number.
        seed: 0x5EED_0000_0000_0001,
        era: Era::Scavenged,
        art: ArtLicense::ShareAlike,
        out: PathBuf::from(DEFAULT_OUT),
    };
    let mut raw = std::env::args().skip(1);
    while let Some(flag) = raw.next() {
        let mut value = || raw.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--count" => args.count = value()?.parse().map_err(|_| "count must be a number")?,
            "--seed" => args.seed = value()?.parse().map_err(|_| "seed must be a number")?,
            "--out" => args.out = PathBuf::from(value()?),
            "--era" => {
                args.era = match value()?.as_str() {
                    "scavenged" => Era::Scavenged,
                    "dyed" => Era::Dyed,
                    other => return Err(format!("unknown era {other}")),
                }
            }
            // `any` accepts every licence the wardrobe allows; `attribution-only`
            // refuses share-alike art, for travellers headed into a screenshot.
            "--art" => {
                args.art = match value()?.as_str() {
                    "any" => ArtLicense::ShareAlike,
                    "attribution-only" => ArtLicense::AttributionOnly,
                    other => return Err(format!("unknown art licence {other}")),
                }
            }
            "-h" | "--help" => return Err(usage().to_owned()),
            other => return Err(format!("unknown option {other}\n{}", usage())),
        }
    }
    Ok(args)
}

/// A page of links, so a whole cast can be walked through in a browser without
/// pasting anything.
fn contact_page(cast: &[(String, Npc)], era: Era) -> String {
    let mut page = String::from(
        "<!doctype html>\n<meta charset=\"utf-8\">\n<title>Waystation travellers</title>\n\
         <style>body{font:14px/1.5 system-ui,sans-serif;margin:2rem;max-width:60rem}\
         li{margin:.6rem 0}code{color:#555}</style>\n",
    );
    let _ = writeln!(page, "<h1>Waystation travellers ({era:?} era)</h1>\n<ol>");
    for (name, npc) in cast {
        let _ = writeln!(
            page,
            "<li><a href=\"{}\">{name}</a> — {:?}<br><code>{}</code></li>",
            npc.generator_url(),
            npc.art_license(),
            npc.describe()
        );
    }
    page.push_str("</ol>\n");
    page
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    std::fs::create_dir_all(&args.out).map_err(|e| format!("{}: {e}", args.out.display()))?;

    let mut cast = Vec::new();
    for index in 0..args.count {
        // Spread the seeds rather than walking 1, 2, 3: neighbouring seeds
        // otherwise open a run with several travellers who share a body base.
        let seed = args
            .seed
            .wrapping_add(index.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let npc = generate_for(seed, args.era, args.art);
        let name = format!("npc-{index:03}.json");
        let path = args.out.join(&name);
        std::fs::write(&path, npc.to_ulpc_json())
            .map_err(|e| format!("{}: {e}", path.display()))?;
        println!("{name}  [{:?}] {}", npc.art_license(), npc.describe());
        println!("           {}", npc.generator_url());
        cast.push((name, npc));
    }

    let index = args.out.join("index.html");
    std::fs::write(&index, contact_page(&cast, args.era))
        .map_err(|e| format!("{}: {e}", index.display()))?;
    let encumbered = cast
        .iter()
        .filter(|(_, npc)| npc.art_license() == ArtLicense::ShareAlike)
        .count();
    println!("\n{} travellers in {}", cast.len(), args.out.display());
    if encumbered > 0 {
        println!(
            "{encumbered} carry share-alike art — fine in game, not in a flat image. \
             Use --art attribution-only for anyone headed into a screenshot."
        );
    }
    println!("open {} to click through them", index.display());
    Ok(())
}

fn main() -> ExitCode {
    if let Err(message) = run() {
        eprintln!("{message}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
