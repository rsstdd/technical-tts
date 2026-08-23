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


## Naming and criteria

Every `evidence_*` item named in `DELIVERY-PLAN.md` becomes exactly one file under
`gates/<gate>/<review-id>/`, named character for character as the delivery plan names it. A name
that a test or a checklist looks up must be written once and matched literally, never paraphrased.

Each record states its acceptance criterion **before** its result. A criterion written after the
result is not a criterion, because a bar set afterwards cannot fail.

Evidence is committed to Git. `docs/governance/RISK-OPEN-QUESTIONS-DESCOPE.md` OQ-02 and OQ-06
record that this project runs on one machine with no backup for runtime state, so committing
evidence is what protects it from machine loss.
