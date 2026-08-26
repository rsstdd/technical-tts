from __future__ import annotations

import argparse
import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
ANALYZER_PATH = REPOSITORY_ROOT / "scripts/qualification/analyze_wav_variation.py"


def load_analyzer():
    spec = importlib.util.spec_from_file_location("analyze_wav_variation", ANALYZER_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("WAV variation analyzer could not be imported")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


ANALYZER = load_analyzer()


class InputRootTests(unittest.TestCase):
    def test_t4_e0_symlinked_input_root_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            input_root = root / "input"
            input_root.mkdir()
            symlink = root / "input-link"
            symlink.symlink_to(input_root, target_is_directory=True)
            arguments = argparse.Namespace(input_root=symlink, expected_count=1)

            with self.assertRaisesRegex(
                ANALYZER.AnalysisError,
                "input root must be a non-symlink directory",
            ):
                ANALYZER.analyze(arguments)

    def test_t4_e0_symlinked_input_root_parent_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            outside = root / "outside"
            input_root = outside / "runs"
            input_root.mkdir(parents=True)
            symlink_parent = root / "input-link"
            symlink_parent.symlink_to(outside, target_is_directory=True)
            arguments = argparse.Namespace(
                input_root=symlink_parent / "runs",
                expected_count=1,
            )

            with self.assertRaisesRegex(
                ANALYZER.AnalysisError,
                "input root must be a non-symlink directory",
            ):
                ANALYZER.analyze(arguments)


if __name__ == "__main__":
    unittest.main()
