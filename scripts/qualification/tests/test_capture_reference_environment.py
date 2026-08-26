from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
CAPTURE_PATH = (
    REPOSITORY_ROOT / "scripts/qualification/capture_reference_environment.py"
)


def load_capture():
    spec = importlib.util.spec_from_file_location(
        "capture_reference_environment", CAPTURE_PATH
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("reference-environment capture could not be imported")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


CAPTURE = load_capture()


class OutputBoundaryTests(unittest.TestCase):
    def test_t4_e0_symlinked_output_parent_is_refused_before_resolution(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            governed_parent = root / "governed"
            governed_parent.mkdir()
            symlink_parent = root / "governed-link"
            symlink_parent.symlink_to(governed_parent, target_is_directory=True)

            with self.assertRaisesRegex(
                CAPTURE.EnvironmentCaptureError,
                "environment evidence parent contains a symbolic-link component",
            ):
                CAPTURE.atomic_write_json(symlink_parent / "capture.json", {})

        self.assertFalse((governed_parent / "capture.json").exists())


if __name__ == "__main__":
    unittest.main()
