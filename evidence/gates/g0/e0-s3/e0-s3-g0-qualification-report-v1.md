# Gate Review: G0 — E0-S3 reference environment and Chatterbox feasibility v1

- Status: measurement result preserved; progression decision superseded on 2026-08-26 by
  `e0-s3-g0-qualification-decision-v2.md`
- Date/time and timezone: 2026-08-25 through 2026-08-26, Europe/Berlin
- Candidate revision: Git base `07be527c54c8373f74050d63d06e87a43ce4ed69` plus the
  checksummed E0-S3 files in this report
- Candidate artifact checksum: ten-run result SHA-256
  `3bfdb26348c240e5927d45e78c4de00e49d8babe759229b91f3668f9e538bddf`
- Environment ID: `reference-wsl2-d9d550f06b783405`
- Accountable owner: Engineering owner
- Approver: Project owner

## Scope and criteria

The criteria below were fixed by `DELIVERY-PLAN.md` and the E0-S3 task before results were
measured. G0 requires a lawful voice and source, a real network-isolated Chatterbox render, a
supported WAV path, worst single-worker RTF `<= 6.0`, and a cold 60-minute projection
`<= 21,600` seconds. The fixed-seed characterization also requires a checksum-linked human
review of all ten randomized outputs. Any hard-gate failure stops E0-S4 and reopens the
hardware/backend decision.

| Requirement | Story | Test/evidence ID | Result | Artifact link/checksum |
|---|---|---|---|---|
| Approved pinned code, weights, source, and owner voice | E0-S3 | Rights readiness review | Pass | Four v2 records in `evidence/rights/`; checksums in Provenance |
| Real Chatterbox render without network routes | E0-S3 | `t5_e0_real_chatterbox_smoke_render_succeeds_offline` | Pass | Smoke result `db75d4845693fdf5ec28c1a33239e7377e96b73f39b06f4a0f34fae34f969d46` |
| Ten-run fixed-seed behavior characterized | E0-S3 | `evidence_e0_fixed_seed_synthesis_determinism_is_characterized` | Pass | Exact named report in this directory; ten-run result `3bfdb26348c240e5927d45e78c4de00e49d8babe759229b91f3668f9e538bddf`; completed review `9ad19dba45dadb00de480c9493a981481cdce753a80f231f12c53d464e97d012` |
| Worst single-worker RTF `<= 6.0` | E0-S3 | `t5_e0_single_worker_rtf_is_at_or_below_6_0` | **Fail: 14.9804** | Ten-run result above |
| Cold 60-minute projection `<= 21,600` seconds | E0-S3 | `t5_e0_projected_sixty_minute_runtime_is_at_or_below_six_hours` | **Fail: 53,947.516 seconds** | Ten-run result above |
| Worker/cache/master/FFmpeg float WAVs decode completely | E0-S3 | `t4_e0_pipeline_wav_variants_round_trip` | Pass | Actual hound report `3b6019da33dbe4da4d0e48f772bf4c1e03eb642668da80b01a0308015216a4b1`; targeted CI test passed |
| Reference-environment report is complete | E0-S3 | `t5_e0_reference_environment_report_complete` | Pass | Environment record `a0de1723fc7d39ea28c0e7076460358e379b88399b43e82cd85bbcfe511f8695` |

## Measured configuration

- Chatterbox `v0.1.2`, commit `eb90621fa748f341a5b768aed0c0c12fc561894b`;
  model revision `1b475dffa71fb191cb6d5901215eb6f55635a9b6`.
- Owner-only `owner-fallback-v1` voice, approved by
  `rights-voice-owner-fallback-v2`; worker identity SHA-256
  `f5628884678f52de2f3a65ea51c9bc2a86e4f5919044fa9b4340eb62465dc2a9`.
- CPU-only, pool size one, three Torch intra-op threads, one interop thread, seed `42` reset
  before every generation, and the pinned Chatterbox v0.1.2 generation defaults.
- Ubuntu 24.04.4 under WSL 2.7.11.0 on the WSL-visible four-physical/eight-logical-core
  allocation of an AMD Ryzen AI 9 HX 370, with 16,770,523,136 bytes RAM.
- All managed roots resolved to ext4 outside `/mnt/c`; the experiment ran under
  `unshare --user --map-root-user --net` with only loopback and no IP route.
- FFmpeg argument-profile SHA-256
  `eea02478be71c18f0b82bd2ad0a7067a4a3be286c593e85c475ef1f8d5856c45`; ffprobe
  filename-independent argument-profile SHA-256
  `da4a12e2852309c99c4c0bb1167cf03f664615ff57462583a72ec4ae8b961026`.

## Results

The real smoke and all ten persistent-model generations completed. Model load took 18.145
seconds and `/usr/bin/time -v` recorded 6,831,940 KiB maximum resident memory for the ten-run
process. Every output was mono 24 kHz 32-bit IEEE-float WAV with 141,120 frames and a duration
of 5.88 seconds.

The ten containers had ten SHA-256 values, but each had the same decoded-PCM and `data`-chunk
hash. The only varying RIFF chunk was libsndfile's `PEAK` metadata timestamp. Aligned waveform
correlation and log-mel cosine similarity were both `1.0` at minimum, median, and maximum, with
zero alignment lag. This bounded observation does not promise deterministic reconstruction.
First-valid-artifact-wins cache publication remains authoritative, and byte-identical
reconstruction requires the retained selected artifact or an archived segment bundle.

Ross reviewed all ten randomized files sequentially before the mapping key was opened. Every
sample was accepted with no omissions/additions, pronunciation, voice-consistency, pacing,
noise/artifact, or audible-difference findings. This completes the characterization but does not
override either failed performance threshold.

## Provenance

| Input | Identity/revision | URI | SHA-256 |
|---|---|---|---|
| Architecture | Accepted ADR | `docs/adr/ADR-0001-production-rust-study-guide-tts.md` | `862200b4513d0475e1efcdfba55ca3e68bb922b2494d56ee5b337effa548696d` |
| Delivery criteria and reforecast | E0-S3 | `DELIVERY-PLAN.md` | `8faf22a6d5a5e61e88839c8da0112325200a4e50c8ed4decb2b881248734494a` |
| Measured proposed decision | ADR-0002 | `docs/adr/ADR-0002-model-hardware-voice-format-qualification.md` | `5bada66fe25cb4a0940fbe94b9c89af540a2e00160e7f709c315c44c936861c2` |
| Reference environment | E0-S3 | `docs/operations/REFERENCE-ENVIRONMENT.md` | `09b88760258a53e52b6540d60d030d6b7ffcc973a41f6e378ed01a64dc922683` |
| Test-data registry | `chatterbox-smoke-v1` | `docs/testing/TEST-DATA-MANIFEST.md` | `56b2fa747a5bdb38ed55ccdf55cb6cac234376cbb1265d6cf18be488b05db0e0` |
| Code rights | `rights-chatterbox-code-v2` | `evidence/rights/rights-chatterbox-code-v2/record.md` | `4cd78b5f27902a4bcc4b639d36486ed50bfd749256aad149e86d2627b7d08808` |
| Weight rights | `rights-chatterbox-weights-v2` | `evidence/rights/rights-chatterbox-weights-v2/record.md` | `538c3fcc3716f6cfdf557d93931451d6674e7278558d1dffe51f9b09fbd2fbea` |
| Source rights | `rights-qualification-sources-v2` | `evidence/rights/rights-qualification-sources-v2/record.md` | `bc2f123c8b52fc352f1b4ad276e53da9586aedfacbf1b361e7a9f84393af724a` |
| Voice rights | `rights-voice-owner-fallback-v2` | `evidence/rights/rights-voice-owner-fallback-v2/record.md` | `75868ec5e0440ff58a207e1bc5c6e386bd6502d3490918b534dbca734869e184` |
| Qualification harness | `1.0-e0-s3-qualification` | `scripts/qualification/chatterbox_spike.py` | `67153661bd41b6e9b9c38b200265e324397baa70ab35c83e4354ab64e5fffa6a` |
| Environment capture | `1.0-e0-s3-reference-environment` | `scripts/qualification/capture_reference_environment.py` | `2a389393dee6e5a5e2dcfc3725e065c00d5021731abefb292d061925e6bff61e` |
| WAV variation analysis | `1.0-e0-s3-wav-variation` | `scripts/qualification/analyze_wav_variation.py` | `0bd07d43e30ca38b46b0ca8b7dfe691ac717b21e2ac2c4ac0ac8e96374c8871a` |
| Actual WAV probe | Qualification example | `crates/study-tts-testkit/examples/qualification_wav_probe.rs` | `d7ce528672fdf231489728e00d6f5b143b044968182ac13f3d4752b65855671b` |
| CI WAV variants | T4 test | `crates/study-tts-testkit/tests/wav_variants.rs` | `182fce44a50a2e70bc141148b531ab5f7e7b7c63d423dd2abaeb9a736e8898b3` |

## Raw artifacts

All raw artifacts are private, access-restricted, and retained under the governed E0-S3
evidence root while this report or a superseding decision cites them.

| Artifact | Governed location | SHA-256 |
|---|---|---|
| Environment inventory | `governed://technical-tts/e0-s3/2026-08-25/reference-wsl2-d9d550f06b783405/reference-environment-v1.json` | `a0de1723fc7d39ea28c0e7076460358e379b88399b43e82cd85bbcfe511f8695` |
| Smoke result | `governed://technical-tts/e0-s3/2026-08-25/reference-wsl2-d9d550f06b783405/smoke-v4/qualification-result-v1.json` | `db75d4845693fdf5ec28c1a33239e7377e96b73f39b06f4a0f34fae34f969d46` |
| Smoke process measurement | `governed://technical-tts/e0-s3/2026-08-25/reference-wsl2-d9d550f06b783405/smoke-v4-time.txt` | `7c1322d3390de23183225dcbedacb69f78b8d4c1011a5902ef9f6fb15357fb3f` |
| Ten-run result | `governed://technical-tts/e0-s3/2026-08-25/reference-wsl2-d9d550f06b783405/fixed-seed-ten-v1/qualification-result-v1.json` | `3bfdb26348c240e5927d45e78c4de00e49d8babe759229b91f3668f9e538bddf` |
| Ten-run process measurement | `governed://technical-tts/e0-s3/2026-08-25/reference-wsl2-d9d550f06b783405/fixed-seed-ten-v1-time.txt` | `574611e881c6af1c067b9e3e5af00fce393b5f698e2777dd25b16c03b79275bd` |
| Container/PCM analysis | `governed://technical-tts/e0-s3/2026-08-25/reference-wsl2-d9d550f06b783405/fixed-seed-ten-v1/wav-variation-v1.json` | `12d81d1eb1f1094765f201d6413387c2c9a544c240c787870bbff54aec194fe9` |
| Actual hound report | `governed://technical-tts/e0-s3/2026-08-25/reference-wsl2-d9d550f06b783405/fixed-seed-ten-v1/hound-probe-v1.json` | `3b6019da33dbe4da4d0e48f772bf4c1e03eb642668da80b01a0308015216a4b1` |
| Rust-assembled private master | `governed://technical-tts/e0-s3/2026-08-25/reference-wsl2-d9d550f06b783405/fixed-seed-ten-v1/rust-assembled-master.wav` | `ee5da12d64e29271846556c954ec9889fd146f7e2d2f29e14b047d97bfad5b60` |
| Pending blind-review sheet | `governed://technical-tts/e0-s3/2026-08-25/reference-wsl2-d9d550f06b783405/fixed-seed-ten-v1/listening/review-sheet.json` | `5448f3fa9e8d55e0ff4b2b432b0b1390020cbea91e5c5c20a759ea570ad0a9ec` |
| Randomization key, opened after blind review | `governed://technical-tts/e0-s3/2026-08-25/reference-wsl2-d9d550f06b783405/fixed-seed-ten-v1/listening/randomization-key.json` | `bae46560450fd5cfd0e918f95b697e1e7e383439024cdacd367eb3c560634cd7` |
| Completed listening review | `governed://technical-tts/e0-s3/2026-08-26/reference-wsl2-d9d550f06b783405/fixed-seed-ten-listening-review-v1/listening-review-v1.json` | `9ad19dba45dadb00de480c9493a981481cdce753a80f231f12c53d464e97d012` |

## Open findings

| Finding | Severity | Owner | Required action | Deadline |
|---|---|---|---|---|
| CPU performance exceeds both hard limits | Blocking | Engineering/project owner | Select and qualify a superseding hardware/backend configuration; do not begin E0-S4 | Before E0-S4 |
| No qualified backup reference machine exists | Accepted risk | Engineering owner | Rebuild the pinned environment and rerun critical qualification within eight working hours after primary loss | Before M3 |

## Decision

- [ ] Pass
- [ ] Conditional pass
- [x] **Fail**

G0 fails because worst RTF and the 60-minute projection exceed hard thresholds. The passing
listening review does not change either failure. ADR-0002 remains Proposed; M2 and M3 forecasts
are blocked; E0-S4 must not begin until a superseding hardware/backend configuration passes G0.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Recorded from the measured hard gates | Fail | 2026-08-25 |
| Project owner | Ross Todd | Acknowledgement pending | — |

## Post-report audit note

`e0-s3-audit-remediation-v1.md` records the later harness, evidence-identity, terminology, and
risk-table corrections. It does not alter this report's measurements or failed performance
result, and it explicitly preserves the limitations of the historical raw artifact.
