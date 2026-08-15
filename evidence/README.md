# Evidence Records

This directory contains redacted, immutable evidence indexes and reports. Raw models, voices, private lessons, corpora, and generated audio remain in governed external locations and are referenced by stable URI and checksum.

Use the templates under `docs/templates/` and follow `docs/testing/EVIDENCE-AND-QUALIFICATION.md`.

```text
evidence/
  gates/<gate>/<review-id>/
  qualification/<area>/<run-id>/
  listening/<lesson-id>/<review-id>/
  rights/<record-id>/
  releases/<version>/<release-id>/
```

Never overwrite an accepted report. Create a new record that explicitly supersedes the prior evidence ID.

