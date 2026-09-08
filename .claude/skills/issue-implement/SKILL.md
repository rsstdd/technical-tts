---
name: issue-implement
description: >
  Implement an assigned `rsstdd/technical-tts` issue end to end and return a verified, adversarially
  reviewed, minimal diff: load authority and skills, capture a baseline, map behavior and blast
  radius, drive each criterion with a failing named test, minimize the diff, review it against
  `rust-review`, close every anomaly, run the required verification, and report with per-criterion
  evidence. Stops at no stage short of done. Use on "implement issue #N", "do the story", "make the
  acceptance criteria true", "finish E2-S4".
argument-hint: "<issue number or URL>"
license: MIT
---

# Implementing a story to done

The issue defines what must change; the repository defines how it may change. Optimize for the
smallest behaviorally complete diff: every criterion becomes true, every invariant outside the
authorized change stays true, and every changed line is justifiable in one sentence. A cleaner
redesign is not a better diff.

Green tests are not completion. Completion is evidence per criterion, plus what this repository
additionally charges: an interface-change record where a contract moved, a governed evidence record
where the story names one, and a human listening review where speech changed.

## Fixed parameters here

| Parameter | Value |
|---|---|
| Authority | newest accepted ADR that explicitly supersedes → `docs/adr/ADR-0001-*` → `DELIVERY-PLAN.md` → `AGENTS.md` → `crates/AGENTS.md` → `PRINCIPLES.md` → skills. Proposed authorizes nothing |
| Skills | `ponytail`, `clean-code` always; `rust-review`, `rust-comment`, `rust-production`, `rust-testing` for any Rust; `worker/AGENTS.md` for the Python worker |
| Toolchain | pinned `1.97.1` by `rust-toolchain.toml`; `rust-version = "1.97"`. CI builds and lints **offline**; never assume a crate may be fetched |
| Ask first | any git operation; accepting an ADR amendment, interface-change record, or evidence record; deleting a cache entry, quarantine, job, or published output; weakening any control; work that depends on a Proposed record |

Route every document question through `docs/INDEX.md`. Never resolve a conflict between binding
authorities silently — flag it. Never commit, branch, push, merge, or open a pull request; when a
commit message is wanted, draft it and stop, and end it with its body prose — no trailers.

## Stage 0–1 — Authority, then the contract

Read the current working tree's governance, never a remembered version. Assemble the criteria set
exactly as `issue-plan` §1 describes — issue Tasks and named tests, the `DELIVERY-PLAN.md` story
and gate acceptance, the ADR-0001 sections and their accepted amendments and deviations. If a plan
comment already exists, treat it as a proposal and verify its citations before using it.

Fill one record per criterion before the first edit:

```text
C-<n>:  criterion, quoted, with its source
  Behavior that must become true / behavior that must not move
  Owning crate and layer, and the contract or version it touches
  Identity effect: synthesis, verification, plan, cache, takes, package — moves or does not
  Containment, rights, offline, checksum, or consent invariant in the path
  Named test that proves it: t<tier>_<epic>_<behavior>, with its tier
  Regression evidence that the unchanged behavior is still unchanged
  Quantitative constraint and how it is measured
```

An ambiguous criterion is recorded with the interpretation chosen and the narrowest reading taken,
or halted on if the choice is architectural. Do not begin without every record filled.

## Stage 2 — Baseline before editing

Inspect the branch, staged, unstaged, and untracked work. Pre-existing user changes are preserved:
not reverted, absorbed, or reformatted. If they overlap, work around them and name them in the
report.

Then run, and record verbatim, the narrowest relevant test plus:

```bash
cargo test --offline -p study-tts-testkit --test walking_skeleton --locked
```

This record is the only admissible evidence for later calling a failure pre-existing. A flake is
recorded and named, never dismissed — the E1-S4 evidence record sets the precedent. If the baseline
is red, isolate the issue's work only where that is safe and report the defect.

## Stage 3 — Map, place, and search the blast radius

Trace each criterion through the real call sites. Never design from filenames.

**Placement** follows the crate boundaries in `crates/AGENTS.md` §Routing table: domain types,
versioned schemas, and planning in `study-tts-core`; job, cache, process supervision, assembly,
export, and package in `study-tts-runtime`; parsed configuration and CLI surface in
`study-tts-cli`; fakes, fixtures, fault injection, and shared contract suites in `study-tts-testkit`.
Validate at the earliest authoritative boundary. Never move logic to make a test convenient.

**Blast radius.** Before changing anything shared, search the workspace for it. In this tree the
elements that bite are: any versioned constant (`*_VERSION`, `*_CONTRACT_VERSION`), a file under
`schemas/` and its `PUBLISHED_REQUIRED_SURFACE` row in `crates/study-tts-testkit/tests/schemas.rs`,
a synthesis or plan key input, the worker frames and `worker/`'s protocol tests, a `fixtures/`
file a golden test pins, the named-test list in `DELIVERY-PLAN.md` §Story, and a `docs/INDEX.md`
row. If behavior is poorly understood but must hold, characterize it with a test first.

**Sensitive-value trace.** Follow every secret-shaped value to every exit: `Debug`, `Display`, the
typed error and its diagnostic, `tracing` fields, `events.ndjson`, `job.json`, `manifest.json`,
`quality-report.json`, assertion messages, panics, and committed fixtures. Nothing may carry source
text, spoken text, a voice-reference path, or an absolute host path out through an alternate path.
IDs, hashes, timings, states, and error classes are what gets logged.

Write the short plan — files, tests, order — that the final report is compared against.

## Stage 4 — Failing test, smallest change, focused run

Per criterion: write or strengthen the named test, confirm it fails for the intended reason,
implement the minimum, run it focused (`cargo test --workspace <test_name>`), repeat. Prove the
criterion at the boundary the criterion names; a unit test does not substitute for the package or
recovery proof a T4 criterion demands.

**A test must reject a plausible wrong implementation.** Name the one it rejects. Live examples
from this tree: a cache key that omits a key input and reuses stale audio under a new model; a
timeout that kills the child and leaks its process group; a publish that renames before the fsync;
a required schema field landing on a minor bump; a seed applied after model construction so every
lifetime differs; an edge-silence check that passes on the wrong side. If the wrong implementation
would pass, the test is not yet a test.

**Quantitative criteria are measured, not eyeballed**: byte-identical canonical artifacts across
runs, digest equality, exact refusal variant, no process left after a kill, no network under the
offline suite, T1–T4 inside their `DELIVERY-PLAN.md` §3.2 budgets.

**Production invariants**, from `rust-production` and governance: typed refusals over silent
fallback, fail-closed at the boundary, stage → fsync → `RENAME_NOREPLACE` → fsync parent, advisory
locks with verified stale ownership, path containment before access, discrete checked argv,
canonical ordered bytes and no floats in an identity, major refused / older minor accepted /
unknown field rejected on a schema.

**Generated artifacts.** If schemas moved:

```bash
cargo run --offline --locked --package study-tts-runtime --example generate-schemas
git diff --exit-code -- schemas/
```

Every generated hunk must map to an intended contract change. Unexpected drift is a regression.

**Comments** per `rust-comment`: why, not what; keep the two-sided coupling comment where code
mirrors a governance document; delete what the change made stale.

Keep a running anomaly ledger and verification log as you go. Do not reconstruct it at report time.

## Stage 5–7 — Minimize, review, close

Delete anything not required by a criterion, a regression test, governance, or a generated
artifact. No speculative trait, single-implementation abstraction, widened visibility for a test,
production hook that exists for an assertion, or remnant of an abandoned approach. Then read
`git diff` whole and justify each hunk in one sentence; a hunk that cannot be is removed. Run
`clean-code` and `ponytail` over the changed lines only — cleanup stays inside the changed
responsibility.

Then stop implementing and review the diff as someone else's pull request, applying `rust-review`'s
severity scale and every loaded skill in one pass. Ask what could still make this unsafe to merge
with every test green: a criterion proved by accident, a moved identity nobody declared, a stranded
cache entry or superseded package, a leak through a diagnostic, an unbounded read, a race or lost
cancellation, a containment gap, schema drift, a test that pins the implementation, unrelated churn.

Every anomaly from any stage ends in exactly one state: **fixed**; **pre-existing**, with baseline
evidence; **out of scope**, with the governing rule, left unchanged; or **blocked**, at a named
human-decision gate. Remediate every valid in-scope finding at any severity, then re-verify and
re-read the diff. Repeat until nothing known remains.

## Stage 8 — Required verification

Narrowest first; do not skip the broad run at the end.

```bash
cargo test --workspace <test_name>
cargo fmt --all -- --check
python3 scripts/check-rust-conventions.py
python3 scripts/check-evidence-provenance.py
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --offline --workspace --doc --locked
cargo test --offline -p study-tts-testkit --test walking_skeleton --locked
```

Add what the change class requires from `AGENTS.md` §Verification, any command the issue names,
and — when the worker moved — `python3 -m compileall -q -f worker/study_tts_worker` and
`python3 -m unittest discover --start-directory worker/tests`. The walking skeleton needs `ffmpeg`
and `ffprobe` on `PATH`; if a required check cannot run, its criterion is **BLOCKED**, not PASS,
and the report says exactly what is unverified and why.

Regression proof is the pre-existing tests, the characterization tests from Stage 3, the schema and
contract suites, and the baseline comparison. A passing new test is not regression evidence.

**Done** means: every criterion implemented with test evidence; unchanged behavior evidenced;
required verification passing; no known in-scope finding left; no speculative abstraction or
remnant; schemas and the `DELIVERY-PLAN.md` test list synchronized with the code in the same
change; the interface-change record, evidence record, and `docs/INDEX.md` row written where owed;
the owed human listen stated; and the tree containing only intentional changes.

## Stage 9 — Halt only where governance reserves the decision

Do not ask what the repository can answer. Halt for an ask-first operation above, an architectural
choice the issue does not authorize, destructive handling of pre-existing work, an unresolved
conflict between binding authorities, or a resource that is unavailable locally. Report:

```text
Governing rule:        <exact text and location>
Repository evidence:   <file:line>
Decision required:     <smallest yes/no or A/B>
Work completed:        <stages finished, with verification state>
```

A halt is not an exit from hard implementation work.

## Final report

Concrete evidence only — `file:line`, test names, exact commands, exact outcomes.

- **Outcome** — the behavior that now exists.
- **Per criterion** — `PASS | BLOCKED`, production evidence, test evidence, regression evidence,
  ambiguity resolved.
- **Identity and contract effect** — what moved, what is invalidated or stranded, what was
  recorded and where; or the evidenced statement that nothing moved.
- **Plan diff** — `| Planned | Executed | Reason |`.
- **Verification** — baseline, focused, regression, final, each with its result.
- **Anomaly ledger** — `| Anomaly | Severity | Terminal state | Evidence |`, and whether any
  finding remains.
- **Scope** — files changed, hand-authored versus generated, pre-existing work left untouched,
  cleanup deliberately excluded.
- **Remaining obligations** — the owed listen, evidence signature, or requalification, each with
  its governing rule and whether it blocks completion.

Never report planned work as completed, and never state that a check passed unless it ran.
