#!/usr/bin/env python3
"""Publish flattened licensed runtime art to the private demo release."""

from __future__ import annotations

import argparse
import subprocess
import tarfile
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RUNTIME_ASSETS = ROOT / "runtime-assets"
RELEASE_TAG = "demo-runtime-assets"
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
    args = parser.parse_args()

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
            "gh", "release", "view", RELEASE_TAG, check=False, capture=True
        )
        if release.returncode == 0:
            run(
                "gh",
                "release",
                "upload",
                RELEASE_TAG,
                str(archive_path),
                "--clobber",
            )
            print(f"updated private release {RELEASE_TAG}")
        elif "release not found" in release.stderr.lower():
            run(
                "gh",
                "release",
                "create",
                RELEASE_TAG,
                str(archive_path),
                "--target",
                "main",
                "--title",
                "Waystation licensed demo runtime art",
                "--notes",
                "Flattened runtime-only art for the hosted demo. "
                "Never make this release public with the source repository.",
            )
            print(f"created private release {RELEASE_TAG}")
        else:
            raise SystemExit(release.stderr.strip() or "could not inspect demo release")


if __name__ == "__main__":
    main()
