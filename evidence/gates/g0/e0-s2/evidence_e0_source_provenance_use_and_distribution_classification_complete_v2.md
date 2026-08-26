# Evidence Report: evidence_e0_source_provenance_use_and_distribution_classification_complete_v2

- Governing story/gate: E0-S2 / G0
- Hypothesis or decision: Qualification and release sources have explicit provenance and one
  classification, with private use recorded separately from publication or distribution.
- Owner: Ross Todd (project owner)
- Date/time and timezone: 2026-08-25T20:22:53+02:00, Europe/Berlin
- Environment ID: WSL2 / Ubuntu 24.04; documentation and executable-record review
- Supersedes: `evidence_e0_source_provenance_use_and_distribution_classification_complete`,
  SHA-256 `9e8f35129b1b992ba7d90fcb81eddcde3621fee7eb763bfa9a81b797a85d86ab`

The predecessor remains unchanged. This immutable report supersedes its improper in-place
amendment and re-attests the original acceptance criterion before presenting the current result.

## Acceptance criterion

Stated before the result, per `evidence/README.md`: each qualification and release source is
classified with exactly one value from the eight-value vocabulary in
`docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Classification: owner-authored, public domain,
permissively licensed, commercially or privately licensed, consented voice reference,
evaluation-only, rights review required, or prohibited. Provenance is recorded with a checksum
where the artifact is committed. Intended private use is recorded separately from publication
or distribution rights. An unresolved classification mechanically blocks production release.
The record encodes classification and scope, not a universal legal conclusion about third-party
material.

## Provenance

| Input | Identity/revision | URI | SHA-256 |
|---|---|---|---|
| Superseded report | original report including its recorded amendment | `evidence/gates/g0/e0-s2/evidence_e0_source_provenance_use_and_distribution_classification_complete.md` | `9e8f35129b1b992ba7d90fcb81eddcde3621fee7eb763bfa9a81b797a85d86ab` |
| Source classification record | `rights-qualification-sources-v1` | `evidence/rights/rights-qualification-sources-v1/record.md` | `ef2281f119328e7ed7be9e6334f434b5dd1be5138da9c0b0d2d8f7a8986696b8` |
| Rights policy | eight-value classification and enforcement | `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` | `f7a3fa1635242f0650e088293b0e6a7f490043cf359b0b7912356329453fa7dc` |
| Ratified distribution scope | internal, owner use only | `docs/governance/RELEASE-PROFILES.md` | `bee9fb86528f9dac4769cae92878a0dfb24682ca58f183da3d8182e7dae9ee41` |
| Test-data manifest | acquired E0-S2 inputs; E0-S3 smoke output pending | `docs/testing/TEST-DATA-MANIFEST.md` | `2a15c2130dee762cf4c8efe68e235c6d10ca77bbb1c4343679a497a88b33d998` |

## Procedure

Reviewed every current row in `rights-qualification-sources-v1` against the test-data manifest
and committed fixtures. Confirmed each source has exactly one classification, committed sources
have matching checksums, and the future G0 smoke text is constrained to owner-authored content
without claiming that the E0-S3 render exists. Confirmed private use and publication are separate
permissions and the ratified release profile remains internal, owner use only. Ran the targeted
core rights tests and the `voice_rights` integration suite, including the failing-closed release
gate.

## Results

| Measurement | Threshold | Result | Pass/fail |
|---|---|---|---|
| Sources with exactly one classification | Every current or bound qualification/release source | Two committed fixtures and the bound future G0 smoke text are owner-authored; future third-party sources default to rights review required | Pass |
| Committed-source provenance | Checksum for every committed source | Two of two fixture checksums are recorded and enforced by `t3_e0_registered_fixture_checksums_match_test_data_manifest` | Pass |
| Private use separated from distribution | Explicit, independent scopes | The rights record grants private synthesis/qualification while `RELEASE-PROFILES.md` limits distribution to internal owner use; neither grants publication | Pass |
| Classification vocabulary | All eight values mirrored and unknown values rejected | Five core rights tests passed, including spelling and unknown-value enforcement | Pass |
| Unresolved classification release gate | Named failing-closed test passes | `t4_e0_production_release_rejects_unresolved_content_rights_classification` passed | Pass |
| E0-S2 manifest state | Inputs acquired without claiming E0-S3 output | `chatterbox-smoke-v1` records acquired code, model, and fallback rights; smoke output remains pending E0-S3 | Pass |
| Universal legal conclusion | None encoded | Policy and rights records limit their claims to classification, provenance, and recorded scope | Pass |

**Overall: PASS.**

## Deviations and limitations

The G0 smoke-test text and spoken output do not yet exist. Its binding row requires owner-authored
text at E0-S3; any substitution requires a superseding rights record. The current mechanical gate
protects the provisional content-rights manifest section ahead of E1-S1 production schemas. This
report makes no E0-S3 rendering, quality, or release-readiness claim.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Project owner (approver) | Ross Todd | Approved; supersedes the amended predecessor | 2026-08-25 |
| Qualified reviewer (rights-review role) | Ross Todd | Approved for recorded scope | 2026-08-25 |
