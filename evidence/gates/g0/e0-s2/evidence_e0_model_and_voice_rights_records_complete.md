# Evidence Report: evidence_e0_model_and_voice_rights_records_complete

- Governing story/gate: E0-S2 / G0
- Hypothesis or decision: A lawful voice configuration is available for the intended use — via the pre-authorized owner-recorded fallback — and every model, voice, and corpus artifact has a rights record with a recorded decision before any real voice rendering.
- Owner: Ross Todd (project owner)
- Date/time and timezone: 2026-08-23, America/Boise
- Environment ID: Not applicable (documentation and records review)

## Acceptance criterion

Stated before the result, per `evidence/README.md`: one rights record exists under
`evidence/rights/<record-id>/` for each of the model code, model weights (covering tokenizer and
codec), each planned voice (Nadia, Tom), the fallback owner voice, and the ASR corpora, each
carrying the required fields of the `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`
"Required records" table and a recorded decision; the single-instructor fallback is
pre-authorized as Approved so a lawful voice path exists for G0; and the approver for any use
not explicitly covered by the recorded terms is identified. "Complete" means every record has a
recorded decision, not that every decision is Approved: the story's acceptance requires a lawful
voice configuration or an approved fallback, and model records may lawfully stand at Review
required because their blocking rule ("No real render without approved record") holds until
owner verification at E0-S3.

## Provenance

| Input | Identity/revision | URI | Checksum |
|---|---|---|---|
| Chatterbox code rights record | branch `e0-s2/voice-content-model-legal` | `evidence/rights/rights-chatterbox-code-v1/record.md` | `f14a83ae2aea65cb9100c29fddaeccf28c28274db85165cfc3d9f2fb6a43a2d7` |
| Chatterbox weights rights record | branch `e0-s2/voice-content-model-legal` | `evidence/rights/rights-chatterbox-weights-v1/record.md` | `f45cddee75b40f7ba443974acfde2654042b2b47f2bd2dffc7a08dcef862db30` |
| Fallback owner voice rights record | branch `e0-s2/voice-content-model-legal` | `evidence/rights/rights-voice-owner-fallback-v1/record.md` | `7453f28b1f14a24912bb472e801f09023a3afaad52db4c3a171dd1a0453dff38` |
| Nadia voice rights record | branch `e0-s2/voice-content-model-legal` | `evidence/rights/rights-voice-nadia-v1/record.md` | `a24274f80ce036ef0e6b93621874c59ffcf8d0b04d4a01ea8a656a4a51bcde26` |
| Tom voice rights record | branch `e0-s2/voice-content-model-legal` | `evidence/rights/rights-voice-tom-v1/record.md` | `c8e221e70c12baf9c581e6397c69f6340faf907e721e4f59e7fc5cb318c8d17f` |
| ASR corpora rights record | branch `e0-s2/voice-content-model-legal` | `evidence/rights/rights-asr-corpora-v1/record.md` | `1e4463904466b33a187ac8b055b167ec46ded02acc71a1c0c550da66c084de7a` |
| Rights, data, and artifact policy | branch `e0-s2/voice-content-model-legal` | `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` | `f7a3fa1635242f0650e088293b0e6a7f490043cf359b0b7912356329453fa7dc` |
| Decision and failure routing | branch `e0-s2/voice-content-model-legal` | `docs/governance/ROUTING-TABLES.md` | `d7bbfb6d4f289f1fab5b1b0ea6a153f61fea26faa8db66d8d120c4a2e5a26bb8` |
| Qualification identity table | branch `e0-s2/voice-content-model-legal` | `docs/adr/ADR-0002-model-hardware-voice-format-qualification.md` | `1155c8968cf44b8e805b45a77c02db10826e6406a4d74748f43be2992dec2888` |
| Voice, content, and retention policy | branch `e0-s2/voice-content-model-legal` | `docs/adr/ADR-0004-voice-content-and-retention-policy.md` | `0601326885b0a122d2b85fb161193f0eb0bc7ae0c424bec9f0cce4552fe104c0` |

## Procedure

Reviewed each rights record against the "Required records" row for its artifact type (fields
present, decision recorded, private use and distribution recorded separately). Confirmed the
fallback pre-authorization satisfies E0-S2 task 3 and ADR-0001 §5.2 route 1. Confirmed the
ADR-0002 identity table names the fallback and pending voice records, and the ADR-0004 approval
table rows are filled from these records while the ADR remains Proposed (a Proposed ADR does not
authorize scope; these records feed its later acceptance). Confirmed the not-covered-use
approver is recorded. Reproduction: `ls evidence/rights/` and `sha256sum` of each record above.

## Results

| Measurement | Threshold | Result | Pass/fail |
|---|---|---|---|
| Model artifact records with recorded decision | 2 (code, weights) | 2, both Review required with a named verification procedure and deferred checksum rows | Pass |
| Voice records with recorded decision | 3 (fallback, Nadia, Tom) | 3: fallback Approved for recorded scope; Nadia and Tom Review required naming the fallback as substitute | Pass |
| Lawful voice path for G0 | Required | Pre-authorized owner-recorded fallback (`rights-voice-owner-fallback-v1`) | Pass |
| ASR corpora access/retention/deletion/backup rules recorded | Required | `rights-asr-corpora-v1` Data handling section; referenced by `docs/testing/TEST-DATA-MANIFEST.md` and ADR-0004 | Pass |
| Model, tokenizer, codec, voice, conditional, and license identities recorded | Required | Recorded or explicitly deferred-with-procedure in the records and ADR-0002; conditional and reference checksums are structural fields of `profile.json` enforced at load | Pass |
| Approver for uses not explicitly covered | Named | Project owner/rightsholder per `docs/governance/ROUTING-TABLES.md` decision rows, cited in each record | Pass |
| Consent-gating enforcement tests | 4 named tests passing | `t4_e0_missing_voice_consent_blocks_profile_load`, `t4_e0_unapproved_voice_profile_cannot_enter_preview_or_production`, `t4_e0_voice_checksum_mismatch_blocks_use`, `t4_e0_voice_records_that_are_not_regular_files_are_refused` pass in `crates/study-tts-testkit/tests/voice_rights.rs` | Pass |
| Recorded consent scope enforced, not merely recorded | Required | `validate_profile_for_use` refuses a use absent from the consent record's `permitted_use` list; `t1_e0_uses_outside_the_recorded_consent_scope_are_refused` | Pass |

**Overall: PASS.**

## Deviations and limitations

The Chatterbox code and weights records stand at Review required: this environment is offline,
so the license texts were drafted from publicly known publication and must be verified by the
owner against the pinned revision at first acquisition (E0-S3) before any real render. This is
a recorded decision with a blocking rule, not a gap. Project owner, rightsholder, and
rights-review roles are held by the same person during solo development; each approval names its
role separately per `docs/governance/PROJECT-EXECUTION-CHARTER.md`.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Project owner (approver) | Ross Todd | Approved | 2026-08-23 |
| Rightsholder (fallback voice) | Ross Todd | Approved | 2026-08-23 |

## Amendments

| Date | Change | Authority |
|---|---|---|
| 2026-08-24 | `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Enforcement gained one row, naming `t4_e0_voice_records_that_are_not_regular_files_are_refused` for the newly mechanized rule that a required voice record which is not a regular file refuses profile load. The Provenance checksum for that document moves from `f23e8f065d61d590a280b39e718dc73478d29b8e20e13d9c85cb99f15719471c` to `f7a3fa1635242f0650e088293b0e6a7f490043cf359b0b7912356329453fa7dc`. | Project owner, amending in place under the approval recorded above |
| 2026-08-24 | The "Consent-gating enforcement tests" measurement rises from three named tests to four, adding `t4_e0_voice_records_that_are_not_regular_files_are_refused`, so the Results table matches the amended §Enforcement table it is read against. `DELIVERY-PLAN.md` §E0-S2 carries the same name in its test roster. | Project owner, re-attesting the measurement on a run of 2026-08-24 |

Recorded rather than applied silently, because `evidence/README.md` holds accepted reports
immutable and prescribes a superseding record instead. The project owner judged both changes to
fall within the scope already approved and directed the in-place amendment.

The Acceptance criterion and Procedure are unchanged, and no threshold was lowered: the
consent-gating measurement rose from three named tests to four. All four ran on 2026-08-24 in
`crates/study-tts-testkit/tests/voice_rights.rs` and pass.
