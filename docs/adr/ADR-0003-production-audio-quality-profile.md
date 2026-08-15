# ADR-0003: Production Audio Quality Profile

- **Status:** Proposed; awaiting calibration
- **Owner:** Engineering owner
- **Approver:** Listener representative and project owner
- **Depends on:** ADR-0001, E2-S3, E5-S1

## Decision to be completed

Freeze the production silence threshold, discontinuity threshold, edge-conditioning limits, canonical codec settings, loudness targets, and one calibrated LUFS reference per voice-profile hash and style.

## Fixed constraints from ADR-0001

- Rust owns silence insertion, concatenation, edge analysis, padding, and ramps.
- Edges use 5 ms RMS frames.
- Each exposed edge has at least 10 ms zero padding.
- A raised-cosine transition ramp is no longer than 5 ms when required.
- Float PCM must satisfy `max(abs(sample)) <= 1.0`.
- WAV is the canonical master; M4A and MP3 derive independently from it.
- Preview loudness references remain provisional and cannot become production references without calibration.

## Calibration table

| Parameter | Candidate | Evidence | Frozen value |
|---|---|---|---|
| Silence RMS threshold | TBD | TBD | Pending |
| Join discontinuity threshold | TBD | TBD | Pending |
| Master loudness target/range | TBD | TBD | Pending |
| True-peak ceiling | TBD | TBD | Pending |
| M4A codec arguments | TBD | TBD | Pending |
| MP3 codec arguments | TBD | TBD | Pending |
| Voice/style LUFS references | TBD per hash/style | TBD | Pending |

## Acceptance

Accept after representative segment and long-form measurements pass, listener review approves joins and dynamics, and unrelated retakes cannot change unrelated segment gain decisions.

