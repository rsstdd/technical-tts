# Evidence and Qualification Protocol

## Evidence standard

Every evidence result records:

- unique evidence ID and governing story/gate;
- hypothesis or decision supported;
- exact software, model, voice, hardware, and platform identities;
- input artifact URIs and checksums;
- procedure and commands without secrets;
- raw-result artifact locations and checksums;
- measurements, uncertainty, and pass/fail threshold when applicable;
- deviations and known limitations;
- reviewer, approver, timestamp, and retention.

Reports are immutable. A rerun creates a new evidence ID and may supersede an older result without overwriting it.

## Qualification routing

| Qualification | Protocol owner | Required evidence | Decision record |
|---|---|---|---|
| Chatterbox viability | Engineering owner | Offline smoke, load, RAM, single-worker RTF, WAV, determinism | ADR-0002 |
| Voice acceptability | Project owner/listener | Consent, checksum, blind listening, permitted scope | ADR-0002/0004 |
| Audio profile | Audio owner/listener | Silence, transition, loudness, codecs, representative listening | ADR-0003 |
| ASR release control | Verification owner/human-review owner | Corpus manifest, seeded defects, confusion rates, determinism | ADR-0005 |
| Long-form release | Engineering/project owner | Soak, drift, memory/handle trends, package integrity, review | Gate record |
| Clean installation | Engineering owner | Fresh WSL2 environment, pinned restore, doctor, build | Release record |

## Listening protocol

Use a quiet environment and the named playback equipment. Review the segment alone and within both joins. Record content accuracy, protected-term pronunciation, naturalness, voice consistency, discontinuities, loudness, unexpected continuation, and package navigation. Do not accept a finding without segment ID, artifact checksum, reviewer, and disposition.

## ASR corpus control

External corpus bytes require an immutable governed location with URI, checksum manifest, license, access, and retention metadata. Seeded defects need documented construction and human validation that splice artifacts do not make detection artificially easy. Patterns enter the expected-ASR lattice only after a listener confirms correct pronunciation.

## Evidence directory convention

```text
evidence/
  gates/<gate>/<review-id>/
  qualification/<area>/<run-id>/
  listening/<lesson-id>/<review-id>/
  rights/<record-id>/
  releases/<version>/<release-id>/
```

Commit redacted Markdown/JSON records only when they contain no sensitive source or voice data. Store raw audio, corpora, model artifacts, and private paths outside Git and reference them by governed URI and checksum.

