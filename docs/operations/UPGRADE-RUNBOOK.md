# Upgrade and Compatibility Runbook

Use this runbook for Rust/Python dependencies, Chatterbox, model/tokenizer/codec artifacts, voice conditionals, worker protocol, schemas, FFmpeg, ASR, audio profiles, and persisted artifact formats.

## Before upgrade

1. Identify the exact current and proposed identities.
2. Read licenses, advisories, upstream changes, and platform requirements.
3. Run the compatibility impact command from E3-S5 when implemented; until then, produce a manual impact report.
4. Identify synthesis-key, verification-key, plan, takes, manifest, schema, and cache effects.
5. Snapshot manifests and checksums for representative retained artifacts.
6. Define downgrade compatibility and rollback.

## Qualification

| Change | Minimum requalification |
|---|---|
| Rust dependency without durable-byte effect | T1–T4 and advisory/license review |
| Python worker source or lockfile | Worker-bundle invalidation, contract suite, real render |
| Chatterbox/model/tokenizer/codec | ADR-0002 amendment, rights, RTF, determinism, listening |
| Voice reference/conditional | New profile hash, consent, loudness calibration, listening |
| Worker protocol/schema | Version/migration, fake and real contracts, old fixtures |
| FFmpeg/build | WAV conversion, codecs, structural validation, audio profile review |
| ASR stack/model/decoder | Verification-key invalidation and ADR-0005 requalification |
| Normalizer/protected terms | Golden review, plan/take impact, ASR pattern impact |
| Audio profile/frozen reference | ADR-0003 amendment and affected-release impact |

## Execution

1. Upgrade lockfiles and declarations in one focused change.
2. Update recorded identities and generated artifacts.
3. Run old-fixture compatibility tests and new qualification.
4. Produce a dry-run report listing stale plans, takes, verifications, and packages.
5. Require explicit acceptance before regenerating or replacing selected audio.
6. Preserve old cached artifacts until accepted references and rollback needs are resolved.

## Rollback

Restore declarations and lockfiles, verify prior checksums, and reopen prior manifests without claiming compatibility for artifacts created under the failed identity. Never rewrite an old cache entry or verification result under a new identity.

## Known compatibility limitations

Standing limitations a persisted-format move has left behind. Each names the change that
introduced it and what clears it.

| Limitation | Introduced by | Clears when |
|---|---|---|
| **Cache retention reporting refuses a workspace holding any pre-`2.0` package.** `prune_candidates` reads every published manifest as a retention root, and a root it cannot decode is an error rather than an empty contribution — so **one** legacy package disables prune reporting for the **whole** workspace, not just for that lesson. | `E2-S2-INTERFACE-CHANGE-001` (`manifest` `1.0` → `2.0`) | Every lesson in the workspace has been rebuilt, so no `1.0-skeleton` manifest remains beneath `previews/`. |

The refusal is deliberate and must not be softened into a skip. Treating an unreadable root as
"references nothing" would report live artifacts as prunable, which is a misleading report today
and data loss once E2-S5 makes prune destructive. `crates/study-tts-runtime/src/prune.rs` names
this section in return.
