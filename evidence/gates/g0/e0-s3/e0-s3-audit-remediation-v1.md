# Audit Remediation: E0-S3 qualification integrity and governance v1

- Governing story/gate: E0-S3 / G0
- Finding source: completion audit of GitHub issue #5
- Owner: Engineering owner
- Date/time and timezone: 2026-08-26, Europe/Berlin
- Status: Complete

## Acceptance criterion

Before this remediation is complete, the qualification harness must hash the actual reference
WAV and conditionals with the workspace-pinned BLAKE3 implementation before model load, require
the exact `voice_qualification` consent scope, and reject either tampered file in automated
tests. A future raw qualification result must bind seed and run count to an experiment identity.
The two rights records must use the official `PerTh` spelling, and the risk register must store
probability independently from acceptance status. Historical raw evidence remains immutable;
the remediation must identify any limitation it cannot repair retroactively.

## Changes and results

| Finding | Remediation | Verification | Result |
|---|---|---|---|
| Actual voice files were not BLAKE3-verified by the spike harness | `chatterbox_spike.py` invokes the qualification-only Rust helper for both governed files and compares the returned digests with the approved profile before importing Chatterbox or loading conditionals | Tampered-reference and tampered-conditionals tests; current governed files independently checked against both BLAKE3 profile values and SHA-256 rights values | Pass |
| Consent scope was not enforced | Harness requires exact `voice_qualification` membership in `permitted_use` before synthesis | Missing-scope regression test; governed consent scope check | Pass |
| Seed and run count were absent from raw identity | Raw schema `1.1-e0-s3-qualification` records an `experiment_identity` binding seed, run count, input SHA-256, worker-identity SHA-256, and generation parameters under its own SHA-256 | Identity-field and seed-invalidation regression tests | Pass for future results |
| Two rights records spelled the watermark `Perth` | Corrected both uncommitted v2 records to the upstream spelling `PerTh` | Exact-text search | Pass |
| Risk probability contained acceptance status | Added a separate `Status` column and restored probability-only values | Markdown-table inspection | Pass |

The current governed `owner-fallback-v1` reference matched approved BLAKE3
`b57455db4712257ab102af210098ef8b0592d03c296178640c6e47ef129c61db` and SHA-256
`1d6b2c247f9e66e23e9d27819920430993ae2296c138dd88a4b39a8f38b117e8`. Its conditionals
matched approved BLAKE3
`4951f9e1fb8a665321b2a31c0eb1691e318378bbf892aef44bb9e85b23598e47` and SHA-256
`f3dbb5c5ae882079cdfde6dbd599d78ba82347f717414b2f74920080d7785f00`. The governed consent
record contained `voice_qualification`. No private path or artifact bytes entered Git or this
report.

## Implementation provenance

| Input | SHA-256 |
|---|---|
| `scripts/qualification/chatterbox_spike.py` | `f1e843de14721ac46a95a5e3a3ddf7efffc6dac370f043348e84b6c293f3888a` |
| `crates/study-tts-core/examples/qualification_blake3_file.rs` | `006eb37075429ac61c171707c0e57f58039088f8b360426da1203c1589c77141` |
| `scripts/qualification/tests/test_chatterbox_spike.py` | `d58046f404fdd2b66f7da108e596ff78bb990531b191992e63d9eeb8f5f00756` |
| `evidence/rights/rights-chatterbox-weights-v2/record.md` | `538c3fcc3716f6cfdf557d93931451d6674e7278558d1dffe51f9b09fbd2fbea` |
| `evidence/rights/rights-voice-owner-fallback-v2/record.md` | `75868ec5e0440ff58a207e1bc5c6e386bd6502d3490918b534dbca734869e184` |
| `docs/governance/RISK-OPEN-QUESTIONS-DESCOPE.md` | `4f5f42270dc6bb724270ccb2efb31e404a93e41ecdb9ff014795aaa839f20ae3` |

## Verification

- `cargo test -p study-tts-core --example qualification_blake3_file --locked`: one test passed.
- `python3 -m unittest discover -s scripts/qualification/tests -v`: seven tests passed.
- Independent current-artifact check: reference BLAKE3 and SHA-256 matched; conditionals BLAKE3
  and SHA-256 matched; `voice_qualification` consent scope was present.
- Full workspace and documentation verification is recorded in the issue-close comment after
  the final candidate passes.

No speech or audio artifact changed, so the completed ten-file listening review remains the
applicable human-listening evidence.

## Historical limitation

The retained 2026-08-25 raw `qualification-result-v1.json` predates this remediation. It does
not independently bind seed or run count and cannot prove that its BLAKE3 profile values were
computed from the exact files loaded. The signed fixed-seed evidence report records seed `42`,
run count `10`, the matching SHA-256 identities, and the procedure used; the audit found no
evidence of artifact alteration. This remediation does not rewrite that raw artifact or claim a
retroactive guarantee. Any rerun produced by the corrected harness records the new identity and
performs the fail-closed actual-file checks.
