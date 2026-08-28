# Reference Environment and Feasibility Record

- **Status:** Measured constrained development environment; performance targets failed
- **Owner:** Engineering owner
- **Governing story:** E0-S3
- **Decision output:** ADR-0002 accepted with a development-only performance waiver
- **Environment ID:** `reference-wsl2-d9d550f06b783405`
- **Measurement date:** 2026-08-25 (Europe/Berlin)

## Environment inventory

| Field | Recorded value |
|---|---|
| WSL/Windows | WSL `2.7.11.0`; WSL kernel package `6.18.33.2-2`; Windows `10.0.26200.9168` |
| Distribution/kernel | Ubuntu `24.04.4 LTS`; Linux `6.18.33.2-microsoft-standard-WSL2`, x86-64 |
| CPU | AMD Ryzen AI 9 HX 370 with Radeon 890M; 4 visible physical cores from unique `lscpu --parse=socket,core` pairs; 8 logical processors |
| RAM/swap | 16,770,523,136 bytes RAM; 4,294,967,296 bytes swap |
| Linux volume at capture | ext4 on `/dev/sdd`; 1,081,101,176,832 bytes capacity; 963,045,920,768 bytes available at the model-root observation |
| Rust | `rustc 1.97.1 (8bab26f4f 2026-07-14)`; Cargo `1.97.1`; checked-in toolchain SHA-256 recorded in raw environment evidence |
| Python | CPython `3.12.3`, GCC `13.3.0`, glibc `2.39`; frozen inventory SHA-256 `7de742701305fd95810a46bf575dc3c18377e5c910f9f48159f256f3e4af48e2` |
| GCC/CMake | GCC `13.3.0`; CMake `3.28.3` |
| FFmpeg/ffprobe | Ubuntu FFmpeg/ffprobe `6.1.1-3ubuntu5`; complete build configuration retained in raw evidence |
| Chatterbox code | `v0.1.2`, commit `eb90621fa748f341a5b768aed0c0c12fc561894b`; installed package SHA-256 `ffcc84b2d199002683f70525f7ee9f5c340a399e840c6eb10d2b876ea576be4d` |
| Qualification worker identity | SHA-256 `f5628884678f52de2f3a65ea51c9bc2a86e4f5919044fa9b4340eb62465dc2a9` |
| Model/tokenizer/codec | `ResembleAI/chatterbox` revision `1b475dffa71fb191cb6d5901215eb6f55635a9b6`; per-file SafeTensors/JSON checksums in `rights-chatterbox-weights-v2` |
| Voice profile | `owner-fallback-v1`; approved by `rights-voice-owner-fallback-v2`; conditional SHA-256 `f3dbb5c5ae882079cdfde6dbd599d78ba82347f717414b2f74920080d7785f00` |

The complete private environment record is
`governed://technical-tts/e0-s3/2026-08-25/reference-wsl2-d9d550f06b783405/reference-environment-v1.json`,
SHA-256 `a0de1723fc7d39ea28c0e7076460358e379b88399b43e82cd85bbcfe511f8695`.

## Root verification

The capture canonicalized repository, qualification environment, model, voice, cache, jobs,
staging, output, and raw-evidence roots. Every root resolved to ext4 on `/dev/sdd`; none had a
symlink component, resolved through `/mnt/c`, or used DrvFS. Raw absolute paths remain in the
private environment record and are deliberately redacted here.

## Qualification controls

- CPU device, pool size one, three Torch intra-op threads, and one interop thread.
- Python, NumPy, and Torch reseeded to `42` immediately before every generation.
- Chatterbox `from_local` only; precomputed conditionals loaded through
  `torch.load(..., weights_only=True)`.
- Repetition penalty `1.2`, minimum probability `0.05`, top probability `1.0`, exaggeration
  `0.5`, CFG weight `0.5`, and temperature `0.8`.
- `HF_HUB_OFFLINE=1`, `TRANSFORMERS_OFFLINE=1`, and
  `HF_HUB_DISABLE_PROGRESS_BARS=1` in an `unshare --user --map-root-user --net` namespace that
  exposed only loopback and no IP route.
- Output explicitly written as mono 24 kHz 32-bit IEEE-float WAV.

## Qualification results

| Measurement | Gate | Result | Decision |
|---|---:|---:|---|
| Offline real smoke render | Pass | Pass | Pass |
| Persistent ten-run backend reliability | 10/10 without failure | 10/10 | Pass |
| Model load time | Record | 18.145 seconds | Recorded |
| `/usr/bin/time -v` peak RSS | Record | 6,831,940 KiB | Recorded |
| Worst single-worker RTF | `<= 6.0` | `14.9804` | **Fail** |
| Cold 60-minute projection | `<= 21,600` seconds | 53,947.516 seconds (14.99 hours) | **Fail** |
| Worker media | Mono, 24 kHz, 32-bit IEEE float | 141,120 frames; 5.88 seconds on every run | Pass |
| Container byte hashes | Characterize | 10 unique SHA-256 values | Recorded |
| Decoded PCM/data hashes | Characterize | 1 unique decoded-PCM hash and 1 unique `data`-chunk hash | Recorded |
| Container variation source | Characterize | `PEAK` metadata timestamp only | Recorded |
| Duration variance | Characterize | min/max 5.88 seconds; population standard deviation 0 | Recorded |
| Aligned waveform correlation | Characterize | min/median/max `1.0`, lag 0 | Recorded |
| Log-mel cosine similarity | Characterize | min/median/max `1.0` | Recorded |
| Randomized ten-file listening | Required characterization | 10/10 accepted; no required-category findings or audible differences | Pass |
| Actual hound path | Decode worker/cache/master/FFmpeg variants | Every sample decoded, finite, in range; formats matched | Pass |

The ten-run result is
`governed://technical-tts/e0-s3/2026-08-25/reference-wsl2-d9d550f06b783405/fixed-seed-ten-v1/qualification-result-v1.json`,
SHA-256 `3bfdb26348c240e5927d45e78c4de00e49d8babe759229b91f3668f9e538bddf`.
The hound report SHA-256 is
`3b6019da33dbe4da4d0e48f772bf4c1e03eb642668da80b01a0308015216a4b1`.
The immutable completed listening review is
`governed://technical-tts/e0-s3/2026-08-26/reference-wsl2-d9d550f06b783405/fixed-seed-ten-listening-review-v1/listening-review-v1.json`,
SHA-256 `9ad19dba45dadb00de480c9493a981481cdce753a80f231f12c53d464e97d012`.
The generated ten-run result remains unchanged and therefore retains its original
`listening_review_complete: false` snapshot; the separately checksummed review completes that
evidence without rewriting the raw synthesis record.

The committed redacted records are
`evidence/gates/g0/e0-s3/e0-s3-g0-qualification-report-v1.md` and
`evidence/gates/g0/e0-s3/evidence_e0_fixed_seed_synthesis_determinism_is_characterized_v2.md`,
which supersedes the original fixed-seed report only to clarify its procedural wording.
The fixed-seed characterization passes, including its checksum-linked human review. That result
does not change the independent performance failure. The superseding progression decision is
`evidence/gates/g0/e0-s3/e0-s3-g0-qualification-decision-v3.md`, which carries the v2 decision
forward with corrected provenance.

## FFmpeg and ffprobe identity

FFmpeg executable SHA-256 is
`ed16af623947494a72e284b6eb8ff225f2da22b38b5d5069c2fd4b4ba3384e41`; ffprobe executable
SHA-256 is `272f6ebc634a63d9c8b4ca68e964119d980f25154e5aa2c35e5487da48e9a58f`.
The exact conversion argv after the executable was:

```json
["-nostdin","-hide_banner","-loglevel","error","-y","-i","run-01.wav","-map_metadata","-1","-ac","1","-ar","24000","-c:a","pcm_f32le","ffmpeg-pcm-f32le.wav"]
```

Its canonical JSON argument-profile SHA-256 is
`eea02478be71c18f0b82bd2ad0a7067a4a3be286c593e85c475ef1f8d5856c45`.

The exact ffprobe argv after the executable for run one was:

```json
["-v","error","-select_streams","a:0","-show_entries","stream=codec_name,sample_fmt,sample_rate,channels,bits_per_sample,duration","-show_entries","format=format_name,duration,size","-of","json","run-01.wav"]
```

Its canonical JSON SHA-256 is
`25ded7ddac05521104e44c4bea2d76defbf08a60938d89d6bea273fcde07ed98`.
Runs two through ten used the identical checked argument prefix and the respective final
basenames `run-02.wav` through `run-10.wav`. The filename-independent profile replaces that
last element with `<input-wav-name>` and has SHA-256
`da4a12e2852309c99c4c0bb1167cf03f664615ff57462583a72ec4ae8b961026`.

## Cache and reconstruction decision

Cache publication remains first-valid-artifact-wins. Even though decoded PCM was identical in
this ten-run sample, a synthesis key identifies a request rather than guaranteed bytes.
Byte-identical reconstruction therefore requires retaining the selected cache artifact or an
archived segment bundle. A seed, request identity, or takes file alone is insufficient.

## Reforecast and decision

Both CPU performance measurements failed on this WSL2 allocation. Accepted ADR-0002 classifies
the allocation as a constrained development environment and waives the failures only as blockers
to development progression. At the E0-S3 decision, E0-S4 could begin while overall G0 remained
open pending its provisional contract baseline. E0-S4 has since supplied that baseline; this does
not change the measured failures or the waiver. The measured values remain authoritative for local
estimates. The intended full-box deployment configuration must pass both unchanged targets before
G3 acceptance.

No qualified backup machine exists. The accepted single-machine risk carries a target of eight
working hours to rebuild the pinned environment and rerun critical qualification on replacement
hardware before M3. This recovery target is not evidence that a replacement machine is already
qualified.
