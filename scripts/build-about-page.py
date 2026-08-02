#!/usr/bin/env python3
"""Build the About page the web shell links to.

Attribution has been accumulating in three places that a player cannot read:
`docs/NPC_ART_CREDITS.md` for the LPC sprites, `assets-manifest.json` for the
licensed audio, and prose in `THIRD_PARTY_ASSETS.md` for what the APIs do. The
game itself is the wrong place to say any of it — a traveller standing in the
doorway should not be interrupted to name a licence — so it goes here, on a
page beside the game rather than inside it.

The point of generating it is that the wardrobe changes. Sprites get added and
dropped, and a hand-written credits page silently stops matching the art it
credits. This one is built from the same records the art is built from, and CI
fails if the committed page has fallen behind them.
"""

from __future__ import annotations

import argparse
import html
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CREDITS = ROOT / "docs" / "NPC_ART_CREDITS.md"
MANIFEST = ROOT / "assets-manifest.json"
OUTPUT = ROOT / "web" / "about.html"

# Where each audio creator's work came from. These are the URLs the licence
# notes beside the purchased audio actually record, copied here rather than
# read from `music/`, which is gitignored and absent on a CI checkout. A
# creator the manifest names and this does not stops the build.
AUDIO_SOURCES = {
    "AndriiG": "https://pixabay.com/users/andriig-54059306/",
    "Dragon Studio": "https://pixabay.com/sound-effects/nature-relaxing-rain-444802/",
}

# What each audio role is, said the way somebody who has played it would say it
# rather than the way the manifest keys it.
AUDIO_ROLES = {
    "background_music": "Music",
    # Indoor rain is the same recording behind a wall, so it is the same credit.
    "rain": "Rain",
    "rain_indoors": "Rain",
    "floorboard_creak": "Floorboards",
    "hammering": "Hammering",
}


def read_credits(path: Path) -> dict[str, object]:
    """Pull the wardrobe's credits file apart into the pieces the page shows.

    The file is generated and its shape is stable, so this parses the four
    headings it is known to have and refuses anything else. A silent miss here
    would drop artists off the page, which is the one failure this whole script
    exists to prevent.
    """
    text = path.read_text(encoding="utf-8")

    revision = re.search(r"revision `([0-9a-f]+)`", text)
    if not revision:
        raise SystemExit(f"{path}: no generator revision recorded")

    # The file says up front how much it is about to name. Everything below is
    # checked against that, so a row this parser fails to recognise is a build
    # failure rather than an artist quietly missing from the page.
    declared = re.search(r"(\d+) people to name across (\d+) source files", text)
    if not declared:
        raise SystemExit(f"{path}: no headcount to check the parse against")
    people, files = (int(count) for count in declared.groups())

    sections: dict[str, list[str]] = {}
    heading = None
    for line in text.splitlines():
        if line.startswith("## "):
            heading = line[3:].strip()
            sections[heading] = []
        elif heading is not None:
            sections[heading].append(line)

    def bullets(name: str) -> list[str]:
        if name not in sections:
            raise SystemExit(f"{path}: expected a '## {name}' section")
        return [
            line[2:].strip() for line in sections[name] if line.startswith("- ")
        ]

    authors = bullets("Authors")
    if len(authors) != people:
        raise SystemExit(
            f"{path}: says {people} people, {len(authors)} parsed out of it"
        )

    rows = []
    for line in sections.get("Per-file", []):
        if not line.startswith("| ") or line.startswith("| ---"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split(" | ")]
        if len(cells) != 5 or cells[0] == "Art":
            continue
        rows.append(cells)
    if len(rows) != files:
        raise SystemExit(
            f"{path}: says {files} source files, {len(rows)} parsed out of it"
        )

    named = {name.strip() for row in rows for name in row[1].split(",")}
    unlisted = sorted(named - set(authors))
    if unlisted:
        raise SystemExit(
            f"{path}: credited per-file but absent from the author list: "
            + ", ".join(unlisted)
        )

    return {
        "revision": revision.group(1),
        "authors": fold_aliases(authors, rows),
        "relied_on": bullets("Licences relied on"),
        "declined": bullets("Also offered upstream, and declined"),
        "rows": rows,
    }


def alias_keys(name: str) -> set[str]:
    """Every form of a name that would identify the same person.

    `Eliza Wyatt (ElizaWy)` is the artist's name beside their handle, so either
    half identifies them.
    """
    keys = {name.lower().strip()}
    both = re.match(r"^(?P<name>.*?)\s*\((?P<handle>[^()]*)\)$", name)
    if both:
        keys.add(both.group("name").lower().strip())
        keys.add(both.group("handle").lower().strip())
    return keys


def fold_aliases(authors: list[str], rows: list[list[str]]) -> list[str]:
    """Count each artist once, however many ways the LPC catalogue spells them.

    Upstream carries `bluecarrot16` and `Bluecarrot16`, and `ElizaWy` both bare
    and beside her name. Listing those as four people on a page whose whole
    purpose is naming the right people would be a small lie about a large
    kindness, so they are merged. Nobody is dropped: the merge only ever joins
    names that already identify the same person, and the form kept is the
    fullest one — a name beside a handle says more about who made this than the
    handle alone, however much more often the handle is written.
    """
    used = re.compile(r"\s*,\s*")
    frequency: dict[str, int] = {}
    for row in rows:
        for name in used.split(row[1]):
            frequency[name.strip()] = frequency.get(name.strip(), 0) + 1

    groups: list[dict[str, object]] = []
    for name in authors:
        keys = alias_keys(name)
        for group in groups:
            if group["keys"] & keys:
                group["names"].append(name)
                group["keys"] |= keys
                break
        else:
            groups.append({"names": [name], "keys": keys})

    def fullest(name: str) -> tuple[bool, int, int]:
        return ("(" in name, frequency.get(name, 0), len(name))

    return sorted(
        (max(group["names"], key=fullest) for group in groups),
        key=str.lower,
    )


def read_audio(path: Path) -> list[dict[str, str]]:
    """Group the licensed audio by creator, keeping the order the manifest has."""
    manifest = json.loads(path.read_text(encoding="utf-8"))
    by_creator: dict[str, list[str]] = {}
    for entry in manifest["licensed_audio"]:
        creator = entry["creator"]
        role = AUDIO_ROLES.get(entry["role"], entry["role"].replace("_", " "))
        roles = by_creator.setdefault(creator, [])
        if role not in roles:
            roles.append(role)
    missing = sorted(creator for creator in by_creator if creator not in AUDIO_SOURCES)
    if missing:
        raise SystemExit(
            "no source page recorded for " + ", ".join(missing) + "; add it to "
            "AUDIO_SOURCES so the credit points somewhere"
        )
    return [
        {"creator": creator, "roles": roles, "source": AUDIO_SOURCES[creator]}
        for creator, roles in by_creator.items()
    ]


LINK = re.compile(r"\[link\]\((?P<url>[^)]+)\)")


def cell_to_html(cell: str) -> str:
    """Render one table cell: backticked paths, `[link](url)` pairs, plain text."""
    urls = [match.group("url") for match in LINK.finditer(cell)]
    if urls:
        # Several pieces have two upstream homes. Numbering them beats printing
        # "source source" and leaving the reader to guess they differ.
        return " ".join(
            f'<a href="{html.escape(url, quote=True)}" target="_blank" '
            f'rel="noopener noreferrer">'
            f'source{f" {index}" if len(urls) > 1 else ""}</a>'
            for index, url in enumerate(urls, start=1)
        )
    escaped = html.escape(cell)
    # A path may break after a slash and nowhere else, so `squarepack` stays a
    # word instead of becoming `squarepac k` in a narrow column.
    escaped = escaped.replace("/", "/<wbr>")
    return re.sub(r"`([^`]+)`", r"<code>\1</code>", escaped)


def build(credits: dict[str, object], audio: list[dict[str, str]]) -> str:
    authors = credits["authors"]
    rows = credits["rows"]

    author_items = "\n".join(
        f"          <li>{html.escape(name)}</li>" for name in authors
    )
    relied_on = ", ".join(html.escape(name) for name in credits["relied_on"])
    declined = ", ".join(html.escape(name) for name in credits["declined"])

    table_rows = "\n".join(
        "            <tr>"
        + "".join(f"<td>{cell_to_html(cell)}</td>" for cell in row)
        + "</tr>"
        for row in rows
    )

    audio_items = "\n".join(
        f"          <li><a href=\"{html.escape(entry['source'], quote=True)}\" "
        f'target="_blank" rel="noopener noreferrer">'
        f"{html.escape(entry['creator'])}</a> — "
        f"{html.escape(', '.join(entry['roles']).lower())}.</li>"
        for entry in audio
    )

    return f"""<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <meta
      name="description"
      content="Who made the art, the music, and the words in The Waystation at the Edge of the Ash."
    />
    <title>About — The Waystation at the Edge of the Ash</title>
    <style>
      @font-face {{
        font-family: "EB Garamond";
        src: url("runtime-assets/fonts/EBGaramond-Variable.ttf") format("truetype");
        font-style: normal;
        font-weight: 400 800;
        font-display: swap;
      }}
      :root {{
        color-scheme: dark;
        background: #151613;
        color: #eadcb9;
        font-family: "EB Garamond", Georgia, "Times New Roman", serif;
      }}
      body {{
        margin: 0;
        padding: 2.5rem 1.25rem 5rem;
        line-height: 1.55;
        background:
          radial-gradient(circle at 50% 0, #293023 0, #151613 45%, #090a08 100%)
          no-repeat;
        background-attachment: fixed;
      }}
      main {{
        max-width: 46rem;
        margin: 0 auto;
      }}
      h1 {{
        margin: 0 0 0.35rem;
        font-size: clamp(1.8rem, 5vw, 2.6rem);
        font-weight: 600;
      }}
      h2 {{
        margin: 2.75rem 0 0.6rem;
        padding-bottom: 0.3rem;
        border-bottom: 1px solid #3d3a2c;
        font-size: 1.45rem;
        font-weight: 600;
      }}
      h3 {{
        margin: 1.75rem 0 0.4rem;
        font-size: 1.1rem;
        font-weight: 600;
        color: #cbb98c;
      }}
      p {{
        margin: 0.8rem 0;
      }}
      a {{
        color: #d8c390;
        text-decoration-color: #6f603f;
      }}
      a:hover,
      a:focus-visible {{
        color: #f4e6c2;
      }}
      .lede {{
        margin-bottom: 2rem;
        color: #a99a77;
        font-size: 1.05rem;
      }}
      .back {{
        display: inline-block;
        margin-bottom: 2rem;
        padding: 0.35rem 0.9rem 0.45rem;
        border: 1px solid #6f603f;
        border-radius: 0.2rem;
        background: #23241d;
        text-decoration: none;
      }}
      blockquote {{
        margin: 1rem 0 1rem 0;
        padding-left: 1rem;
        border-left: 2px solid #4d4636;
        color: #cbb98c;
        font-style: italic;
      }}
      ul.names {{
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(15rem, 1fr));
        gap: 0.1rem 1.5rem;
        margin: 0.8rem 0;
        padding-left: 1.15rem;
      }}
      ul.plain {{
        margin: 0.8rem 0;
        padding-left: 1.15rem;
      }}
      ul.plain li {{
        margin: 0.35rem 0;
      }}
      .scroller {{
        max-height: 34rem;
        overflow: auto;
        margin: 1rem 0;
        border: 1px solid #3d3a2c;
        border-radius: 0.2rem;
        background: #1b1c17;
      }}
      table {{
        width: 100%;
        border-collapse: collapse;
        font-size: 0.85rem;
      }}
      thead th {{
        position: sticky;
        top: 0;
        z-index: 1;
        padding: 0.5rem 0.7rem;
        border-bottom: 1px solid #4d4636;
        background: #23241d;
        text-align: left;
        white-space: nowrap;
      }}
      td {{
        padding: 0.4rem 0.7rem;
        border-bottom: 1px solid #26271f;
        vertical-align: top;
      }}
      td:first-child {{
        min-width: 11rem;
      }}
      tbody tr:last-child td {{
        border-bottom: none;
      }}
      code {{
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
        font-size: 0.9em;
        color: #cbb98c;
        overflow-wrap: break-word;
      }}
      .footnote {{
        margin-top: 3rem;
        color: #8c8267;
        font-size: 0.9rem;
      }}
    </style>
  </head>
  <body>
    <main>
      <a class="back" href="./">← Back to the Waystation</a>

      <h1>About</h1>
      <p class="lede">
        Who made the art, the music and the words, and what the game is doing
        when a traveller tells you what is wrong.
      </p>

      <h2>Scripture</h2>
      <p>
        The verses on the cards and in the little book are the
        <strong>Berean Standard Bible</strong>, which its translation committee
        dedicated to the public domain on 30 April 2023. Attribution is
        appreciated and not required, so here it is:
      </p>
      <blockquote>
        The Holy Bible, Berean Standard Bible, BSB is produced in cooperation
        with Bible Hub, Discovery Bible, OpenBible.com, and the Berean Bible
        Translation Committee.
      </blockquote>
      <p>
        When the game is running with a key for the
        <a href="https://developers.youversion.com/" target="_blank" rel="noopener noreferrer">YouVersion&nbsp;Platform&nbsp;API</a>,
        a traveller's passage arrives in the language your browser asks for,
        from an edition chosen for that language. Those words are fetched for
        that one card and are not stored here. Editions other than the Berean
        Standard Bible carry their own terms, so the card names the edition
        beside the reference; the public-domain BSB asks for nothing and is
        left unnamed. Without a key, the game serves reviewed English wording
        held in this repository and credits nobody it should not.
      </p>

      <h2>What listens</h2>
      <p>
        A traveller says what is wrong in their own words. Deciding which need
        that is, and which of the reviewed passages answers it, is done by a
        model reached through the
        <a href="https://www.gloo.com/" target="_blank" rel="noopener noreferrer">Gloo AI Platform</a>.
        It chooses from a fixed set of passages that were selected and read in
        advance — it is choosing which one to offer, not writing Scripture, and
        it cannot offer anything that is not on the list. If it cannot be
        reached, the game falls back to reviewed wording written by hand, and
        plays exactly as it otherwise would.
      </p>

      <h2>Music and sound</h2>
      <p>Every sound in the game comes from two people.</p>
      <ul class="plain">
{audio_items}
      </ul>

      <h2>The people you meet</h2>
      <p>
        Everyone who walks up the road is assembled from the
        <a href="https://lpc.opengameart.org/" target="_blank" rel="noopener noreferrer">Liberated Pixel Cup</a>
        sprite collection, through the
        <a href="https://github.com/LiberatedPixelCup/Universal-LPC-Spritesheet-Character-Generator" target="_blank" rel="noopener noreferrer">Universal LPC Spritesheet Generator</a>
        at revision <code>{credits["revision"]}</code>.
        {len(authors)} people made the {len(rows)} pieces of art below, and all
        {len(authors)} are named here.
      </p>
      <p>
        Waystation is not open source, so it cannot take art offered only under
        the GPL. Every piece below is used under one of the other licences its
        authors also offered it under: {relied_on}. The offers not taken, listed
        so the choice is on the record: {declined}.
      </p>

      <h3>Artists</h3>
      <ul class="names">
{author_items}
      </ul>

      <h3>Piece by piece</h3>
      <p>
        <em>Used under</em> is the licence Waystation takes.
        <em>Also offered</em> is the rest of what the authors offered.
      </p>
      <div class="scroller" tabindex="0">
        <table>
          <thead>
            <tr>
              <th>Art</th><th>Authors</th><th>Used under</th>
              <th>Also offered</th><th>Where it came from</th>
            </tr>
          </thead>
          <tbody>
{table_rows}
          </tbody>
        </table>
      </div>

      <p class="footnote">
        This page is generated from the same records the art is built from, so
        it cannot fall behind what the game actually ships.
      </p>
    </main>
  </body>
</html>
"""


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if the committed page is not what this would write",
    )
    parser.add_argument("--output", type=Path, default=OUTPUT)
    args = parser.parse_args()

    page = build(read_credits(CREDITS), read_audio(MANIFEST))

    if args.check:
        current = args.output.read_text(encoding="utf-8") if args.output.exists() else ""
        if current != page:
            print(
                f"{args.output} is out of date; run `make about` and commit it",
                file=sys.stderr,
            )
            return 1
        print(f"{args.output} is current")
        return 0

    args.output.write_text(page, encoding="utf-8")
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
