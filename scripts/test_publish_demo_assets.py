#!/usr/bin/env python3
"""Tests for the licensed demo-runtime packager."""

from __future__ import annotations

import importlib.util
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

SCRIPT = Path(__file__).with_name("publish-demo-assets.py")
SPEC = importlib.util.spec_from_file_location("publish_demo_assets", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class PublishDemoAssetsTests(unittest.TestCase):
    def test_archive_contains_only_runtime_files_under_runtime_root(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            runtime = root / "runtime-assets"
            (runtime / "world").mkdir(parents=True)
            (runtime / "world" / "tree.png").write_bytes(b"tree")
            (runtime / "provenance.json").write_text("{}\n", encoding="utf-8")
            archive = root / "demo.tar.gz"

            with patch.object(MODULE, "ROOT", root), patch.object(
                MODULE, "RUNTIME_ASSETS", runtime
            ):
                count = MODULE.build_archive(archive)

            self.assertEqual(count, 2)
            with tarfile.open(archive, "r:gz") as package:
                self.assertEqual(
                    sorted(package.getnames()),
                    [
                        "runtime-assets/provenance.json",
                        "runtime-assets/world/tree.png",
                    ],
                )

    def test_empty_runtime_tree_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            runtime = root / "runtime-assets"
            runtime.mkdir()
            with patch.object(MODULE, "ROOT", root), patch.object(
                MODULE, "RUNTIME_ASSETS", runtime
            ), self.assertRaisesRegex(SystemExit, "empty"):
                MODULE.build_archive(root / "demo.tar.gz")


if __name__ == "__main__":
    unittest.main()
