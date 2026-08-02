#!/usr/bin/env python3
"""Publish flattened licensed runtime art and audio to the private asset repository.

The source repository is public, and a Release inherits its repository's
visibility with none of its own. This archive therefore cannot live beside the
code: as a single tarball at a stable URL it is an asset pack, downloadable
without the game around it, which is the one form these licences plainly refuse.

It lives in a separate private repository instead, and default-branch CI pulls it
in with a token to build the browser bundle. The flattened art is then served
from that bundle, which is ordinary — every game ships its assets. What never
happens is handing over the pack.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import tarfile
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RUNTIME_ASSETS = ROOT / "runtime-assets"
RELEASE_TAG = "demo-runtime-assets"
ASSET_REPO_ENV = "WAYSTATION_ASSET_REPO"
ARCHIVE_NAME = "waystation-runtime-assets.tar.gz"


def run(
    *command: str, check: bool = True, capture: bool = False
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=ROOT,
        check=check,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.PIPE if capture else None,
    )


def build_archive(destination: Path) -> int:
    files = sorted(path for path in RUNTIME_ASSETS.rglob("*") if path.is_file())
    if not files:
        raise SystemExit("runtime-assets is empty; run the asset build first")
    if any(path.is_symlink() for path in RUNTIME_ASSETS.rglob("*")):
        raise SystemExit("runtime-assets must not contain symbolic links")

    with tarfile.open(destination, "w:gz") as archive:
        for path in files:
            archive.add(path, arcname=path.relative_to(ROOT), recursive=False)
    return len(files)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="build and inspect the archive without changing the GitHub release",
    )
    parser.add_argument(
        "--repo",
        default=os.environ.get(ASSET_REPO_ENV),
        help="private OWNER/NAME to hold the pack; defaults to $" + ASSET_REPO_ENV,
    )
    args = parser.parse_args()
    if not args.repo and not args.dry_run:
        raise SystemExit(
            f"--repo, or ${ASSET_REPO_ENV}, must name a private repository.\n"
            "Publishing this archive to the public source repository would be "
            "distributing the asset pack itself."
        )
    target = ["--repo", args.repo] if args.repo else []

    run("python3", "scripts/build-assets.py", "--strict-private")
    with tempfile.TemporaryDirectory(prefix="waystation-demo-assets-") as temp:
        archive_path = Path(temp) / ARCHIVE_NAME
        file_count = build_archive(archive_path)
        print(
            f"packaged {file_count} flattened runtime files "
            f"({archive_path.stat().st_size} bytes)"
        )
        if args.dry_run:
            return

        release = run(
            "gh", "release", "view", RELEASE_TAG, *target, check=False, capture=True
        )
        if release.returncode == 0:
            run(
                "gh",
                "release",
                "upload",
                RELEASE_TAG,
                str(archive_path),
                "--clobber",
                *target,
            )
            print(f"updated private release {RELEASE_TAG}")
        elif "release not found" in release.stderr.lower():
            run(
                "gh",
                "release",
                "create",
                RELEASE_TAG,
                str(archive_path),
                *target,
                "--target",
                "main",
                "--title",
                "Waystation licensed demo runtime assets",
                "--notes",
                "Flattened runtime-only art and selected audio for the hosted demo. "
                "This repository must stay private; the game is served from the "
                "bundle CI builds out of this, never from the pack itself.",
            )
            print(f"created private release {RELEASE_TAG}")
        else:
            raise SystemExit(release.stderr.strip() or "could not inspect demo release")


if __name__ == "__main__":
    main()
