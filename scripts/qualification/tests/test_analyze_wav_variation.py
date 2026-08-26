from __future__ import annotations

import argparse
import importlib.util
import sys
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from unittest.mock import patch


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


@dataclass(frozen=True)
class SoundFileInfo:
    format: str = "WAV"
    subtype: str = "FLOAT"
    frames: int = 1
    duration: float = 1 / 24_000


class RejectingSoundFile:
    def __init__(self, info: SoundFileInfo) -> None:
        self._info = info
        self.info_called = False
        self.read_called = False

    def info(self, path: Path) -> SoundFileInfo:
        self.info_called = True
        return self._info

    def read(self, path: Path, *, dtype: str, always_2d: bool):
        self.read_called = True
        raise AssertionError("rejected qualification WAV must not be decoded")


class Samples:
    shape = (1, 1)

    def astype(self, dtype: str, *, copy: bool):
        return self

    def tobytes(self, *, order: str) -> bytes:
        return b"\x00\x00\x00\x00"


class FiniteSamples:
    def all(self) -> bool:
        return True


class Numpy:
    @staticmethod
    def isfinite(samples: Samples) -> FiniteSamples:
        return FiniteSamples()


class AcceptingSoundFile:
    def __init__(self) -> None:
        self.inputs: list[bytes] = []

    def info(self, source) -> SoundFileInfo:
        self.inputs.append(source.read())
        return SoundFileInfo()

    def read(self, source, *, dtype: str, always_2d: bool):
        self.inputs.append(source.read())
        return Samples(), ANALYZER.SAMPLE_RATE_HZ


class WavInputBoundaryTests(unittest.TestCase):
    def test_t4_e0_analysis_uses_one_bounded_wav_state(self) -> None:
        raw = b"RIFF\x10\x00\x00\x00WAVEdata\x04\x00\x00\x00\x00\x00\x00\x00"
        with tempfile.TemporaryDirectory() as temporary_directory:
            path = Path(temporary_directory) / "run-01.wav"
            path.write_bytes(raw)
            soundfile = AcceptingSoundFile()

            with patch.object(
                ANALYZER,
                "read_bounded_wav",
                wraps=ANALYZER.read_bounded_wav,
            ) as read_bounded_wav:
                ANALYZER.analyze_wav(path, soundfile, Numpy())

        read_bounded_wav.assert_called_once_with(path)
        self.assertEqual(soundfile.inputs, [raw, raw])

    def test_t4_e0_oversized_wav_is_rejected_before_media_inspection(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            path = Path(temporary_directory) / "run-01.wav"
            with path.open("wb") as output:
                output.truncate(ANALYZER.MAX_WAV_BYTES + 1)
            soundfile = RejectingSoundFile(SoundFileInfo())

            with self.assertRaisesRegex(
                ANALYZER.AnalysisError,
                "qualification WAV exceeds the configured input limits",
            ):
                ANALYZER.analyze_wav(path, soundfile, object())

        self.assertFalse(soundfile.info_called)
        self.assertFalse(soundfile.read_called)

    def test_t4_e0_riff_reader_rejects_oversized_wav_before_reading(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            path = Path(temporary_directory) / "run-01.wav"
            with path.open("wb") as output:
                output.truncate(ANALYZER.MAX_WAV_BYTES + 1)

            with self.assertRaisesRegex(
                ANALYZER.AnalysisError,
                "qualification WAV exceeds the configured input limits",
            ):
                ANALYZER.read_riff_chunks(path)

    def test_t4_e0_non_float_wav_is_rejected_before_decoding(self) -> None:
        for info in (
            SoundFileInfo(format="AIFF"),
            SoundFileInfo(subtype="PCM_16"),
        ):
            with self.subTest(format=info.format, subtype=info.subtype):
                with tempfile.TemporaryDirectory() as temporary_directory:
                    path = Path(temporary_directory) / "run-01.wav"
                    path.write_bytes(b"not decoded")
                    soundfile = RejectingSoundFile(info)

                    with self.assertRaisesRegex(
                        ANALYZER.AnalysisError,
                        "qualification WAV has an unexpected media format",
                    ):
                        ANALYZER.analyze_wav(path, soundfile, object())

                self.assertTrue(soundfile.info_called)
                self.assertFalse(soundfile.read_called)

    def test_t4_e0_frame_and_duration_limits_are_enforced_before_decoding(self) -> None:
        invalid_metadata = (
            SoundFileInfo(frames=ANALYZER.MAX_WAV_FRAMES + 1),
            SoundFileInfo(
                duration=ANALYZER.MAX_WAV_DURATION_SECONDS + 1 / 24_000,
            ),
        )
        for info in invalid_metadata:
            with self.subTest(frames=info.frames, duration=info.duration):
                with tempfile.TemporaryDirectory() as temporary_directory:
                    path = Path(temporary_directory) / "run-01.wav"
                    path.write_bytes(b"not decoded")
                    soundfile = RejectingSoundFile(info)

                    with self.assertRaisesRegex(
                        ANALYZER.AnalysisError,
                        "qualification WAV exceeds the configured input limits",
                    ):
                        ANALYZER.analyze_wav(path, soundfile, object())

                self.assertTrue(soundfile.info_called)
                self.assertFalse(soundfile.read_called)


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
