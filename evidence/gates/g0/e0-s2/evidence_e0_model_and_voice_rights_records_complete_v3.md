# Evidence Report: evidence_e0_model_and_voice_rights_records_complete_v3

- Governing story/gate: E0-S2 / G0
- Hypothesis or decision: A pinned and rights-approved Chatterbox bundle and an acquired,
  consented owner voice provide the lawful fallback required before any real lesson rendering.
- Owner: Ross Todd (project owner and fallback-voice rightsholder)
- Date/time and timezone: 2026-08-25T20:22:53+02:00, Europe/Berlin
- Environment ID: WSL2 / Ubuntu 24.04 / Python 3.12.3 / CPU-only extraction
- Supersedes: `evidence_e0_model_and_voice_rights_records_complete_v2`, SHA-256
  `cbc6a10c454ad5313d50ec6ff53a1349923af6d0c3b98b53dbe274b234d210da`

The predecessor remains unchanged. This report restores the Delivery Plan's strict completion
bar by recording verified licenses, immutable artifact identities, and an acquired fallback
profile rather than treating deferred verification or a planned recording as complete.

## Acceptance criterion

Stated before the result, per `evidence/README.md`: one rights record exists under
`evidence/rights/<record-id>/` for the Chatterbox code, model weights (including tokenizer and
codec), Nadia, Tom, the fallback owner voice, and the ASR corpora; each record carries the fields
required by `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` and a recorded decision. Before any
real lesson render, the selected Chatterbox code and model artifacts must have exact revisions,
verified license bytes, local checksums, and approval for the intended use. The lawful voice path
must be acquired, selected, consented, checksummed, safely loadable, and accepted by the runtime
voice gate. If Nadia or Tom remains unavailable, their Review-required records must explicitly
select the approved owner-recorded fallback. Uses outside the recorded scope must name an
approver. A pre-authorization, deferred checksum, or unresolved model license does not satisfy
this criterion.

## Provenance

| Input | Identity/revision | URI | SHA-256 |
|---|---|---|---|
| Superseded report | `evidence_e0_model_and_voice_rights_records_complete_v2` | `evidence/gates/g0/e0-s2/evidence_e0_model_and_voice_rights_records_complete_v2.md` | `cbc6a10c454ad5313d50ec6ff53a1349923af6d0c3b98b53dbe274b234d210da` |
| Chatterbox code rights | `rights-chatterbox-code-v2` | `evidence/rights/rights-chatterbox-code-v2/record.md` | `e2850972d83c6fb85d0a0c489582110f95f6cb214a1328d9abea52488d0b743f` |
| Chatterbox artifact rights | `rights-chatterbox-weights-v2` | `evidence/rights/rights-chatterbox-weights-v2/record.md` | `ea0a1424cb5e4f696ab44aad1d43bcf086fe3b46452f43590acdda9d5aac722d` |
| Acquired fallback voice rights | `rights-voice-owner-fallback-v2` | `evidence/rights/rights-voice-owner-fallback-v2/record.md` | `9ed07599bae960aa1c75139ad7ed194d882617bf8be760283fcf8146bcd5cde4` |
| Nadia voice rights | `rights-voice-nadia-v1` | `evidence/rights/rights-voice-nadia-v1/record.md` | `a24274f80ce036ef0e6b93621874c59ffcf8d0b04d4a01ea8a656a4a51bcde26` |
| Tom voice rights | `rights-voice-tom-v1` | `evidence/rights/rights-voice-tom-v1/record.md` | `c8e221e70c12baf9c581e6397c69f6340faf907e721e4f59e7fc5cb318c8d17f` |
| ASR corpora rights | `rights-asr-corpora-v1` | `evidence/rights/rights-asr-corpora-v1/record.md` | `1e4463904466b33a187ac8b055b167ec46ded02acc71a1c0c550da66c084de7a` |
| Rights policy | current E0-S2 policy | `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` | `f7a3fa1635242f0650e088293b0e6a7f490043cf359b0b7912356329453fa7dc` |
| Qualification identity table | candidate identities; qualification Pending | `docs/adr/ADR-0002-model-hardware-voice-format-qualification.md` | `86ac449598a6574e7c154e4e11126c5fa1cfcea3aeba6747276a97a754526f00` |
| Voice policy | acquired fallback; ADR Proposed | `docs/adr/ADR-0004-voice-content-and-retention-policy.md` | `f87f60fe993806c8828fe6cd793c70891c4b83fed16cbf7b66798ac11285c561` |

Governed external inputs are identified in the superseding rights records. Raw models, voice
files, conditionals, package inventories, and rejected takes remain outside Git.

## Procedure

1. Verified Chatterbox release `v0.1.2` resolves to Git commit
   `eb90621fa748f341a5b768aed0c0c12fc561894b`. Reviewed its exact MIT license bytes and the
   pinned model card's MIT declaration before acquisition.
2. Acquired only `s3gen.safetensors`, `t3_cfg.safetensors`, `ve.safetensors`, and
   `tokenizer.json` from model snapshot
   `1b475dffa71fb191cb6d5901215eb6f55635a9b6`. Verified every local SHA-256. Confirmed no
   legacy model `.pt` file or packaged `conds.pt` was acquired.
3. Built the isolated CPU extractor with the recorded 64-package inventory. Loaded the exact
   local model with Hugging Face and Transformers offline flags, a connection-attempt trap, and
   bounded PyTorch threads.
4. Recorded four owner takes without deleting any. The owner rejected takes 1 and 3, listened to
   take 4, and explicitly selected it. FFprobe validated the selected file as 10.833792 seconds,
   24 kHz, mono, 16-bit PCM.
5. Recorded consent for only `private_synthesis` and `voice_qualification`. Generated the
   conditional once using the pinned CPU extractor, then reloaded that exact conditional through
   Chatterbox's `weights_only=True` path with network access trapped.
6. Passed the strict private profile through the existing runtime gate in a disposable Rust
   harness. A deliberately nonexistent FFmpeg executable produced the later `MissingTool`
   failure with synthesis count zero, proving the voice gate accepted the profile without
   producing speech.
7. Ran the named E0-S2 rights and voice tests against the records and runtime enforcement.

## Results

| Measurement | Threshold | Result | Pass/fail |
|---|---|---|---|
| Chatterbox code identity and license | Exact pin and verified approval before acquisition | `v0.1.2`; commit `eb90621fa748f341a5b768aed0c0c12fc561894b`; MIT SHA-256 `4248e910a928849fe5815a0f9236e17fa07768d95b9193212752c464b93d6caa`; Approved for recorded scope | Pass |
| Model identity and license | Exact pin and verified approval before acquisition | Snapshot `1b475dffa71fb191cb6d5901215eb6f55635a9b6`; model-card SHA-256 `c2c75c034eadc6595789724e6b8b3ffcc2025f0875785cafeb9b39e1514e64b6`; Approved for recorded scope | Pass |
| Safe acquired artifacts | Four pinned files; no legacy `.pt` or packaged conditional | Four of four hashes match `rights-chatterbox-weights-v2`; prohibited legacy/package files absent | Pass |
| Dependency identity | Complete resolved inventory and license identities | 64-package freeze SHA-256 `7de742701305fd95810a46bf575dc3c18377e5c910f9f48159f256f3e4af48e2`; license inventory SHA-256 `184fb371bf3d05ed1abbf56e7c62e78363ad8181b8c731f14df60295e4a4e71f`; all hashed RECORD entries verify | Pass |
| Offline exact-model load | Zero network attempts | Loaded at 24 kHz; zero connection attempts; peak RSS 6,826,712 KiB; 3 PyTorch threads and 1 interop thread | Pass |
| Selected owner reference | 8–12 seconds, 24 kHz mono PCM, human approved | Take 4; 10.833792 seconds; SHA-256 `1d6b2c247f9e66e23e9d27819920430993ae2296c138dd88a4b39a8f38b117e8`; listened and approved by speaker/rightsholder | Pass |
| Consent and handling | Granted owner-only scope and protected files | `private_synthesis` and `voice_qualification` only; directory `0700`, required files `0600`; rejected takes preserved | Pass |
| Conditional identity and safe reload | One extraction; target reload uses `weights_only=True`; zero network attempts | SHA-256 `f3dbb5c5ae882079cdfde6dbd599d78ba82347f717414b2f74920080d7785f00`; BLAKE3 `4951f9e1fb8a665321b2a31c0eb1691e318378bbf892aef44bb9e85b23598e47`; one target load with `weights_only=True`; zero connection attempts | Pass |
| Planned voices | Lawful selected fallback when unavailable | Nadia and Tom remain Review required; both records name the approved owner fallback, now acquired as `rights-voice-owner-fallback-v2` | Pass |
| Runtime voice gate | Real private profile accepted before a later controlled failure | Reached deliberate `MissingTool(FFmpeg)` with zero synthesis calls | Pass |
| Named enforcement tests | All pass | 5 core rights tests, 11 core voice tests, and 8 `voice_rights` integration tests passed, including all five Delivery Plan names | Pass |

**Overall: PASS.**

## Deviations and limitations

No spoken TTS output was generated. The only audio played was the owner reference during take
selection. E0-S3 still owns a real Chatterbox render, RTF and resource measurements,
determinism, and audio qualification. ADR-0002 and ADR-0004 remain Proposed/Pending and are not
treated as accepted production decisions. Consent does not authorize publication, distribution,
commercial use, or relabeling this one-speaker profile. Nadia and Tom remain unavailable and
Review required; their lawful substitution is the selected owner fallback, not an implied
approval of either unavailable voice.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Project owner (approver) | Ross Todd | Approved; supersedes v2 without changing it | 2026-08-25 |
| Speaker/rightsholder | Ross Todd | Take 4 and recorded consent scope approved | 2026-08-25 |
