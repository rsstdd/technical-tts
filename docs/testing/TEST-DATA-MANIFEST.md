# Test Data Manifest

Every committed or external test artifact must have a row before use. Checksums alone are insufficient for external corpora; record immutable location, rights, access, retention, and provenance.

| ID | Type | Purpose | Location | Checksum | Rights record | Sensitive | Retention | Owner | Status |
|---|---|---|---|---|---|:---:|---|---|---|
| fixture-tone-v1 | Generated audio | Walking skeleton | Planned `fixtures/audio/` | TBD when created | Owner-generated | No | Repository lifetime | Engineering owner | Planned |
| lesson-two-segment-v1 | Lesson JSON | Walking skeleton | Planned `fixtures/lessons/` | TBD when created | Owner-authored | No | Repository lifetime | Project owner | Planned |
| chatterbox-smoke-v1 | Real-model input/output | G0 qualification | Governed external evidence root | TBD | Approved test voice/source required | Yes | ADR-0004 | Engineering owner | Blocked on rights record |
| asr-clean-corpus-v1 | Human-verified audio | ADR-0005 calibration | Governed external artifact location | TBD manifest | TBD | Possibly | ADR-0004/0005 | Verification owner | Planned |
| asr-seeded-defects-v1 | Derived audio | ADR-0005 calibration | Governed external artifact location | TBD manifest | Derived from approved corpus | Possibly | ADR-0004/0005 | Verification owner | Planned |

## Admission checklist

- [ ] Artifact has a stable ID and checksum.
- [ ] Source and derivation are reproducible.
- [ ] Rights and intended use are recorded.
- [ ] Sensitive data is excluded from Git and ordinary CI.
- [ ] Expected behavior and approving reviewer are recorded.
- [ ] Retention and deletion authority are recorded.
- [ ] Generated defects were checked for artificial cues.

