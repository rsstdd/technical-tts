# Audit Remediation: E0-S3 qualification integrity and governance v2

- Governing story/gate: E0-S3 / G0
- Finding source: completion audit of GitHub issue #5 and follow-up review
- Owner: Engineering owner
- Date/time and timezone: 2026-08-26, Europe/Berlin
- Status: Accepted
- Retention: While the E0-S3 G0 decision or a superseding record cites it;
  predecessor `e0-s3-audit-remediation-v1` is retained unchanged alongside it
- Supersedes: `e0-s3-audit-remediation-v1`, SHA-256
  `75278c0deba3d80fc8ca97e863c3583d483c8e0533d713369b2dbe75ee8c8bea`

The accepted predecessor remains byte-for-byte unchanged. This record carries forward its
remediation result while recording the corrected implementation provenance and follow-up gate
integrity checks.

## Acceptance criterion

Stated before the result, per `evidence/README.md`: every control required by the superseded
remediation must remain implemented and tested. The named T4 WAV-variant gate must fail when
FFmpeg is absent, the WAV analyzer must reject a symbolic link in any existing input-root
component before resolution, and accepted evidence must be preserved rather than edited.
Current provenance must identify the exact reviewed files and their SHA-256 values.

## Superseded evidence

| Input | URI | SHA-256 |
|---|---|---|
| Audit remediation v1 | `evidence/gates/g0/e0-s3/e0-s3-audit-remediation-v1.md` | `75278c0deba3d80fc8ca97e863c3583d483c8e0533d713369b2dbe75ee8c8bea` |
| Fixed-seed evidence v1 | `evidence/gates/g0/e0-s3/evidence_e0_fixed_seed_synthesis_determinism_is_characterized.md` | `8f98b97531994109965ce5bf54712a63e1d6f2b380341b756dab4a71fb64b060` |

## Results

| Control | Verification | Result |
|---|---|---|
| Voice file BLAKE3 verification and `voice_qualification` consent scope | Tamper, missing-scope, and identity regression tests retained from v1 | Pass |
| Seed and run count in future raw identity | Identity-field and seed-invalidation regression tests retained from v1 | Pass |
| Missing FFmpeg cannot pass the named T4 gate | `PATH=/nonexistent` execution failed with an explicit FFmpeg-required diagnostic | Pass |
| Input-root symlink containment | Direct-root and parent-component regression tests | Pass |
| Immutable accepted evidence | Both accepted predecessors match the SHA-256 values in the Superseded evidence table; corrections are in new records | Pass |

## Implementation provenance

| Input | SHA-256 |
|---|---|
| `scripts/qualification/chatterbox_spike.py` | `e90124eecc94e2559a1817c8bdaa6799188c08daf2621ee1f009bbaf4fc8fb1d` |
| `crates/study-tts-core/examples/qualification_blake3_file.rs` | `006eb37075429ac61c171707c0e57f58039088f8b360426da1203c1589c77141` |
| `scripts/qualification/tests/test_chatterbox_spike.py` | `37fe190bc7e81ace5704341213ceba175cc81525b98f28fd34b0bbca3e3e424b` |
| `crates/study-tts-testkit/tests/wav_variants.rs` | `711ceedc5397f47b961cf3a49e1068429898bf3b9476e3dbe0a6eb36d8a291e3` |
| `scripts/qualification/analyze_wav_variation.py` | `3ddff01a90121900f2b369a6bc0c32bac6773d3f8cc58b628e7dc035905d8611` |
| `scripts/qualification/tests/test_analyze_wav_variation.py` | `fe833de1174be9773b9498c1df22b04b600fb6e3de247567f031707d8739900f` |
| `evidence/rights/rights-chatterbox-weights-v2/record.md` | `538c3fcc3716f6cfdf557d93931451d6674e7278558d1dffe51f9b09fbd2fbea` |
| `evidence/rights/rights-voice-owner-fallback-v2/record.md` | `75868ec5e0440ff58a207e1bc5c6e386bd6502d3490918b534dbca734869e184` |
| `docs/governance/RISK-OPEN-QUESTIONS-DESCOPE.md` | `57c477e8fe7d3b4058bf8a79b8bf5ba56c622e4ed2af2c528ed63e89d529f398` |

## Verification

- `cargo test -p study-tts-testkit --test wav_variants
  t4_e0_pipeline_wav_variants_round_trip --locked`: passed with FFmpeg available.
- The compiled named T4 test with `PATH=/nonexistent`: failed as required; the gate did not
  report success.
- `python3 -m unittest scripts.qualification.tests.test_analyze_wav_variation -v`: two tests
  passed.
- `python3 -m unittest discover -s scripts/qualification/tests -v`: ten tests passed.

No speech or audio artifact changed, so the completed ten-file listening review remains the
applicable human-listening evidence. This remediation produced no new raw artifact; the
governed raw results cited by the E0-S3 evidence keep their recorded retention.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner (approver) | Ross Todd | Approved; every control listed above was verified and both accepted predecessors matched the SHA-256 values in the Superseded evidence table | 2026-08-26 |
| Project owner | Ross Todd | Accepted as the follow-up qualification-integrity audit cited by the E0-S3 G0 decision | 2026-08-26 |
