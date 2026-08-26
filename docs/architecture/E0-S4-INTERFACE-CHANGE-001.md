# E0-S4 Interface Change 001 — Validated Cache Tokens and Package Preflight

## Identification

- Record ID: `E0-S4-INTERFACE-CHANGE-001`
- Contract owner: T-AUDIO
- Engineering owner: runtime implementation owner
- Affected-track reviewers: T-CORE, T-RUNTIME, and T-WORKER consumers
- Accepted ADR, if architectural: not applicable; authority boundaries from
  ADR-0001 remain unchanged

## Version and compatibility

- Contract IDs: `cache_publication`, `package_writer`
- Old versions: `e0.cache-publication.0.1`, `e0.package-writer.0.1`
- New versions: `e0.cache-publication.1.0`, `e0.package-writer.1.0`
- Compatibility class: breaking Rust API correction before G1
- Required/defaulted fields: cached-artifact fields become opaque; package
  preflight returns a required prepared-writer session
- Unknown-field behavior: unchanged; project-owned JSON remains strict
- Wire or Rust representation changed: Rust representation only; no durable or
  worker-frame bytes changed

## Impact

- Synthesis identities affected: none
- Verification identities affected: none
- Plan, takes, or package identities affected: none
- Consumers and commands affected: preview orchestration and testkit package and
  cache fakes; no product command exists in E0
- Fakes and shared suites affected: `FakeCachePublisher`, `FakePackageWriter`,
  `run_cache_contract_scenario`, and `run_package_writer_contract_scenario`
- Fixtures and schemas affected: none
- Existing cached artifacts affected: none; the filesystem format and validation
  rules are unchanged
- Published packages or accepted takes affected: none

## Delivery and recovery

- Fake and shared-suite update completed before consumers: regression assertions
  were added first; fake and consumer implementations ship in this one change
- Migration procedure: downstream Rust consumers must obtain cache artifacts from
  `CachePublisher` and call `PackageWriter::preflight` before prepare/write
- Rollback procedure: revert the Rust API and its consumers together before G1;
  no artifact migration or deletion is permitted or required
- Compatibility evidence: unchanged cache and package bytes pass the existing
  walking skeleton; new tests refuse forged ordering, path escape, and late gates
- Mapped tests and qualification rerun: provisional-contract, package-artifact,
  cache containment, and complete walking-skeleton suites
- Walking skeleton result: recorded in the implementation handoff and refreshed
  E0-S4 evidence after the complete suite runs

## Approval

- Contract owner decision: adopted as pre-G1 audit remediation
- Engineering owner approval: implementation requested on 2026-08-26; G1 freeze
  approval remains deferred
- Affected-track approvals: deferred to the G1 fake/real parity review
- Effective version and date: provisional `1.0`, 2026-08-26
