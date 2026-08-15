# Release and Rollback

## Publication boundary

Until OQ-01 is resolved and M3 passes, `publish` must refuse operation. Private-preview builds write only to the configured preview root and state `release_status: private_preview` in their manifest.

## Release prerequisites

- every ADR-0001 acceptance criterion is traced and satisfied;
- ADR-0002 through ADR-0005 are accepted or explicitly amended with equivalent controls;
- release candidate passes the clean-machine and 45–60 minute qualification;
- rights, consent, content classification, retention, and distribution scope are approved;
- dependencies, licenses, advisories, SBOM, model checksums, and worker bundle are recorded;
- every selected take and verification/human-review result is current;
- package files, manifest, checksums, captions, chapters, and codecs validate;
- signing and key custody are approved;
- rollback rehearsal succeeds.

## Release procedure

1. Freeze source, lockfiles, schemas, worker bundle, model identities, voice profiles, and quality profiles.
2. Build on the named clean reference environment.
3. Run required T1–T6 suites and collect immutable reports.
4. Verify gate evidence against the release candidate manifest.
5. Generate SBOM, dependency/license inventory, checksums, and signatures.
6. Perform final human listening and navigation review.
7. Obtain role-specific approvals.
8. Publish to the approved target.
9. Verify the published bundle independently.
10. Store the release index and rollback reference.

## Rollback triggers

- checksum or signature mismatch;
- wrong source, voice, take, or release status;
- rights or consent withdrawal;
- corrupted or incomplete package;
- severe audio, content, privacy, or security defect;
- inability to reconstruct the selected cached artifacts;
- release evidence later shown to be stale or invalid.

## Rollback procedure

1. Stop further publication and mark the release withdrawn in the release index.
2. Preserve the affected bundle, manifest, checksums, logs, and approvals as incident evidence.
3. Restore the previously verified bundle or disable distribution when restoration is unsafe.
4. Verify restored checksums and consumer-visible state.
5. Identify affected caches, previews, and downstream copies through manifest references.
6. Create corrective issues and an incident record.
7. Re-release only through the complete release procedure.

Rollback never rewrites Git history, deletes evidence, or silently substitutes audio under an existing version.

