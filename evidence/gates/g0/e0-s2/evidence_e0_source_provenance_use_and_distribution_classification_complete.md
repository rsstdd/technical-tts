# Evidence Report: evidence_e0_source_provenance_use_and_distribution_classification_complete

- Governing story/gate: E0-S2 / G0
- Hypothesis or decision: Every qualification and release source carries exactly one recorded classification with provenance, and intended private use is recorded separately from publication or distribution rights, so no source can reach a release with its rights unresolved or its scope implicit.
- Owner: Ross Todd (project owner)
- Date/time and timezone: 2026-08-23, America/Boise
- Environment ID: Not applicable (documentation and records review)

## Acceptance criterion

Stated before the result, per `evidence/README.md`: each qualification and release source is
classified with exactly one value from the eight-value vocabulary of
`docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Classification (owner-authored, public
domain, permissively licensed, commercially or privately licensed, consented voice reference,
evaluation-only, rights review required, prohibited); its provenance is recorded with a
checksum where the artifact is committed; intended private use is recorded separately from any
publication or distribution right; and an unresolved classification blocks production release
mechanically. The record encodes classification and scope only, not a universal legal
conclusion about third-party material.

## Provenance

| Input | Identity/revision | URI | Checksum |
|---|---|---|---|
| Qualification and release source classification record | branch `e0-s2/voice-content-model-legal` | `evidence/rights/rights-qualification-sources-v1/record.md` | `ef2281f119328e7ed7be9e6334f434b5dd1be5138da9c0b0d2d8f7a8986696b8` |
| Rights, data, and artifact policy (classification vocabulary and enforcement table) | branch `e0-s2/voice-content-model-legal` | `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` | `f23e8f065d61d590a280b39e718dc73478d29b8e20e13d9c85cb99f15719471c` |
| Ratified distribution scope | branch `e0-s2/voice-content-model-legal` | `docs/governance/RELEASE-PROFILES.md` | `bee9fb86528f9dac4769cae92878a0dfb24682ca58f183da3d8182e7dae9ee41` |
| Test data manifest | branch `e0-s2/voice-content-model-legal` | `docs/testing/TEST-DATA-MANIFEST.md` | `39eebd67e3782717b6413bacc169bd9c1fef1d43ff2b1ffacf25b2405dcb244c` |

## Procedure

Reviewed the classification table of `rights-qualification-sources-v1` against the manifest
rows and committed fixtures. Confirmed the eight-value vocabulary is mirrored one-to-one by
`SourceClassification` in `crates/study-tts-core/src/rights.rs` with a coupling comment on both
sides. Confirmed the ratified distribution scope (internal, owner use only) is referenced, not
restated, and that private use and publication are separate permissions in the record.
Reproduction: `cargo test -p study-tts-core rights` and
`cargo test -p study-tts-testkit --test voice_rights`.

## Results

| Measurement | Threshold | Result | Pass/fail |
|---|---|---|---|
| Sources with exactly one classification | All current sources | 3 of 3 (two committed fixtures, G0 smoke text) classified owner-authored; future third-party sources default to rights review required | Pass |
| Committed sources with checksum provenance | All committed sources | 2 of 2 carry SHA-256 rows in `docs/testing/TEST-DATA-MANIFEST.md`, enforced by `t3_e0_registered_fixture_checksums_match_test_data_manifest` | Pass |
| Private use recorded separately from distribution | Required | Separate Permitted-scope lines in the record; distribution scope defers to `docs/governance/RELEASE-PROFILES.md` §4 (internal, owner use only) with its reopening conditions | Pass |
| Classification vocabulary mirrored in code | 8 of 8 values | `SourceClassification` mirrors the policy list; unknown values are a parse error (`t3_e0_classification_vocabulary_round_trips_and_unknown_values_are_rejected`), and the on-disk spellings are pinned to the serde representation (`t3_e0_classification_spellings_match_their_serde_representation`) | Pass |
| Unresolved classification blocks production release | Named failing-closed test | `t4_e0_production_release_rejects_unresolved_content_rights_classification` passes in `crates/study-tts-testkit/tests/voice_rights.rs` | Pass |
| No universal legal conclusion encoded | Required | Policy scope statement and record rationale state engineering-control scope only | Pass |

**Overall: PASS.**

## Deviations and limitations

The G0 smoke-test text does not exist yet; its classification row binds the E0-S3 author to
owner-authored text, and any substitution routes through a superseding record. The mechanical
release gate covers the provisional `content_rights` manifest section ahead of the E1-S1
versioned schemas; the enforcement, not the field shape, is the claim this evidence supports.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Project owner (approver) | Ross Todd | Approved | 2026-08-23 |
| Qualified reviewer (rights review role) | Ross Todd | Approved | 2026-08-23 |
