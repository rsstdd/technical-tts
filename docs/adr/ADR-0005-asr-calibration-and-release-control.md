# ADR-0005: ASR Calibration and Release Control

- **Status:** Proposed; awaiting E4 evidence
- **Owner:** Verification engineering owner
- **Approver:** Human-review owner and project owner
- **Depends on:** ADR-0001, E4

## Decision to be completed

Record exact `whisper-rs`, `whisper-rs-sys`, `whisper.cpp`, ASR model, build features, device, decoder parameters, thread count, input conversion, expected-pattern profile, normalizer, thresholds, calibration corpus, and measured confusion rates. Decide whether ASR is a production release control.

## Fixed decoder profile

- greedy decoding, `best_of = 1`;
- English, translation disabled;
- `no_context = true`;
- no initial prompt;
- temperature and temperature increment `0.0`;
- explicit thread count;
- one segment per independent decoder state;
- managed 16 kHz mono PCM conversion with recorded FFmpeg identity.

## Acceptance gates

| Measurement | Required |
|---|---:|
| False-positive rate on at least 100 clean human-verified segments | `<= 5%` |
| Protected-term clean segments | At least 50 |
| Seeded examples per defect class | At least 50 |
| Omission detection | `>= 95%` |
| Insertion detection | `>= 95%` |
| Unexpected continuation detection | `>= 95%` |
| Substitution detection | `>= 90%` |
| Repetition detection | `>= 80%` |
| Repeated identical-input transcript | Identical 5/5 |
| Segment-order invariance | 100% |

## Failure path

Failure of any class prevents ASR from becoming a release control. Development may continue with complete human review. Version 1 may ship only if an ADR amendment explicitly replaces automated coverage claims with complete immutable human review and the project owner accepts the schedule and residual risk.

