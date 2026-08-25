# Voice Candidate Register

## Purpose

This register records investigated voice sources before acquisition. It is discovery evidence,
not a rights record, consent grant, backend decision, or qualification result. A candidate may
enter synthesis only after it has its own approved rights or consent record, immutable source
identity and checksums, governed reference profile, and every applicable qualification result.

ADR-0001-D002 selects `owner-fallback-v1` as the only version 1 voice configuration. Nothing in
this register binds a source to the fictional Nadia or Tom role, adds a second production
speaker, authorizes a non-Chatterbox backend, or changes E0-S3.

## Status vocabulary

| Status | Meaning |
|---|---|
| Selected | Approved elsewhere for the recorded scope and selected by an accepted decision |
| Eligible acquisition route | May be investigated after the required consent and rights records are obtained |
| Rights review required | Source information exists, but the project lacks sufficient permission or consent evidence |
| Rejected for this project | Known rights, architecture, provenance, or quality constraints make the candidate unsuitable |

## Current and potential sources

| Candidate | Source and observed identity | Status | Project disposition |
|---|---|---|---|
| Owner recording | `owner-fallback-v1`; selected take 4 and derived Chatterbox conditional are identified by `rights-voice-owner-fallback-v2` | Selected | The only voice authorized for E0-S3 and version 1 qualification. Authorization is limited to owner-only `private_synthesis` and `voice_qualification`. |
| Directly consenting second speaker | New 8–12 second recording commissioned or volunteered specifically for this project | Eligible acquisition route | Obtain a signed consent record covering the speaker, recording rights, voice cloning, intended scope, retention, and revocation before recording or extraction. A future two-speaker format also requires a superseding accepted decision. |
| LJSpeech recordings | Single-speaker English dataset published as public domain; pinned dataset documentation is listed below | Rights review required | Copyright classification alone does not establish the speaker's consent to voice cloning or waive voice, likeness, and personality interests. Do not acquire or extract a profile until those terms receive explicit review. |
| Piper `en_US-ljspeech-high` | Piper model card describes a single US-English female voice trained on LJSpeech | Rights review required | The model metadata is useful provenance, but generated audio is not an approved Chatterbox reference. Piper would also introduce a second TTS backend unless used only in a separately approved acquisition workflow. |
| Piper `en_US-libritts-high` | Piper model card describes 904 speakers trained from LibriTTS `train-clean-360`, licensed CC BY 4.0 | Rights review required | Individual-speaker identity, attribution, dataset terms, and cloning consent remain unresolved. Do not select or relabel a speaker as Nadia or Tom. |
| Microsoft Edge neural voices | Prior local project scripts named Jenny, Guy, Brian, Ava Multilingual, Ryan, and Andrew Multilingual | Rejected for this project | `edge-tts` client licensing does not grant rights to Microsoft voice identities or output. The Edge service has no project-approved cloning/reference-output grant, requires network access, and is not the standard Chatterbox backend. |
| Windows desktop voices | Prior local project scripts named Zira, David, and Mark | Rejected for this project | The prior Python and UV caches contain no reusable reference or model artifacts, and no project-approved license grants Chatterbox cloning from these OS-bundled voices. Their Windows runtime also falls outside the selected WSL2 Chatterbox path. |
| Piper Ryan or Lessac-derived voices | Ryan is CC BY-NC-SA 4.0 and fine-tuned from Lessac; Lessac has separate Blizzard dataset terms | Rejected for this project | The noncommercial, share-alike, and upstream-lineage terms are not suitable for the intended governed cloning workflow. |

## Prior local-project audit

The inspected Windows-project UV caches contain `edge-tts==7.2.8` and a Windows Python 3.12.13
runtime. They do not contain WAV, MP3, M4A, FLAC, Ogg, ONNX, PyTorch, safetensors, checkpoint,
or other reusable voice/model artifacts. The local scripts used the Microsoft voices listed
above through either the networked Edge service or Windows `System.Speech`; they did not define
or retain Nadia or Tom voice assets.

The `edge-tts` package is client software. Its LGPL-3.0-or-later package license must not be
treated as a license for Microsoft voice identities, service access, generated output, or using
that output as a cloning reference.

## Admission rules

- Keep Nadia and Tom as unbound fictional role labels until each has a separately approved,
  provenance-ready profile.
- Do not download candidate audio, models, or generated output on the strength of this register.
- Do not use synthetic output as a Chatterbox reference unless its source terms explicitly
  permit that use and the project approves the complete derivation.
- Public-domain or permissive copyright status does not substitute for recorded speaker consent
  under ADR-0001 and ADR-0004.
- Store any subsequently approved raw reference outside Git under the restricted managed voice
  root; record only governed identifiers, checksums, consent, and approval in the repository.
- Any second speaker reopens the two-speaker architecture and qualification gates through a
  superseding accepted decision as required by ADR-0001-D002.

No candidate in this register is admitted to `docs/testing/TEST-DATA-MANIFEST.md`. No candidate
artifact was downloaded, generated, or copied into the repository during this review.

## Sources reviewed

Reviewed 2026-08-25. These links identify the public source material used for candidate
classification; they are not project approval records.

- [Microsoft Product Terms for Microsoft Azure](https://www.microsoft.com/licensing/terms/en-us/productoffering/MicrosoftAzure/EAEAS) — the explicit prebuilt-neural-voice output grant is stated for customers of the paid-tier TTS service.
- [Microsoft Edge Immersive Reader and Read Aloud](https://support.microsoft.com/en-us/topic/use-immersive-reader-in-microsoft-edge-78a7a17d-52e1-47ee-b0ac-eff8539015e1) — documents the Edge accessibility feature.
- [Microsoft Q&A: commercial use of Edge Read Aloud voices](https://learn.microsoft.com/en-us/answers/questions/5925556/commercial-use-of-edge-read-aloud-voices-via-edge) — records the absence of a public, binding Edge output-use grant and directs users to Microsoft for licensing clarification.
- [Pinned LJSpeech dataset documentation](https://huggingface.co/datasets/keithito/lj_speech/blob/1532a199abd253d1d9511ea04d6d7f45a12a39f9/README.md) — records dataset composition and public-domain publication.
- [Piper `en_US-ljspeech-high` model card](https://huggingface.co/rhasspy/piper-voices/blob/main/en/en_US/ljspeech/high/MODEL_CARD) — records the model's LJSpeech lineage.
- [Piper `en_US-libritts-high` model card](https://huggingface.co/rhasspy/piper-voices/blob/main/en/en_US/libritts/high/MODEL_CARD) — records the multi-speaker LibriTTS lineage and CC BY 4.0 dataset classification.
- [Piper Ryan model card](https://huggingface.co/rhasspy/piper-voices/blob/main/en/en_US/ryan/medium/MODEL_CARD) — records CC BY-NC-SA 4.0 terms and Lessac fine-tuning lineage.
- [Piper Lessac model card](https://huggingface.co/rhasspy/piper-voices/blob/main/en/en_US/lessac/medium/MODEL_CARD) — records the separate Blizzard dataset license lineage.
