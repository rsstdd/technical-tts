# Worker Environment and Lock Procedure

The Python worker's environment is a **synthesis-key input**. ADR-0001 §12.5 derives the
worker-bundle hash from the production worker source, its imported project-owned modules, the
production Python lockfile, the worker protocol schema, launcher configuration that affects
inference, and the Python runtime and platform ABI. §22 adds the rule that shapes everything
below: *derive the worker-bundle hash mechanically; do not depend on a human-managed revision
marker.*

The practical consequence is that this document is not a convenience. A dependency that resolves
differently on two machines produces speech that differs while every cache key stays the same, and
a cache that reuses such an entry ships audio its identity does not describe.

## What the bundle is

`worker/bundle-manifest.json` is the worker's own declaration of what it consists of. It is read by
`crates/study-tts-runtime/src/worker_bundle.rs`, which names this document in return.

| Field | Meaning |
|---|---|
| `schema_version` | Layout of the manifest itself; an unknown value is refused, never guessed at. |
| `import_roots` | Package directories, relative to the repository root, whose modules are project-owned. |
| `inputs` | Every file that belongs to the bundle, relative to the repository root. |
| `python` | Interpreter implementation, version, ABI tag, and platform tag the bundle is resolved for. Checked against the interpreter at `worker/.venv/bin/python`, not trusted (`WORKER_INTERPRETER_PATH` in `crates/study-tts-runtime/src/worker_environment.rs`). |
| `startup_modules` | Interpreter startup code this machine carries that no locked distribution owns, by digest. Added by layout `1.1`; see [Nor are their bytes, nor the modules `site` imports by name](#nor-are-their-bytes-nor-the-modules-site-imports-by-name). |
| `record_digests` | What each locked distribution's `RECORD` must claim, by digest. Added by layout `1.2`; see [Declaring what the lock installed](#declaring-what-the-lock-installed). |

What the manifest does **not** decide is which of these fields, and which files, have to be there at
all. That floor is the next section.

Paths are relative to the **repository root**, not to `worker/`, because ADR-0001 §12.5 counts the
worker protocol schema among the inputs and `schemas/` is a sibling of `worker/`.

### The floor is not the manifest's to set

A manifest that decides its own scope decides its own identity. Two Rust-owned constants in
`crates/study-tts-runtime/src/worker_bundle.rs` take that decision away from it, and this document
is the other end of the mirror they name:

| Constant | What it requires |
|---|---|
| `REQUIRED_BUNDLE_INPUTS` | `worker/bundle-manifest.json`, `worker/launcher.json`, `worker/requirements.lock`, `schemas/worker-protocol-v1.schema.json`, and `worker/study_tts_worker/worker.py` appear in `inputs`. |
| `REQUIRED_IMPORT_ROOT` | `worker/study_tts_worker` appears in `import_roots`. |

Each is an input ADR-0001 §12.5 names one by one, so none of them is optional. Both failures they
close were quiet ones. Dropping `worker/launcher.json` from `inputs` left a bundle that still
hashed, under a key that no longer moved when inference-affecting configuration changed. Emptying
`import_roots` switched the completeness check below off rather than failing it, because a walk with
no directory to walk finds nothing missing. The entrypoint is required as an *input* as well as
being reachable by the walk, for the one case the walk cannot see: a package root that is walked and
is empty because its modules were deleted.

`WorkerBundle::hash` applies both before it reads a single declared input, so a manifest short of
one is refused by what it omitted rather than by whichever of the rest is missing from disk.

### The declaration is checked, not trusted

A hand-written input list proves nothing on its own: adding a module and forgetting to declare it
would leave the hash unchanged while the worker's behavior changed. So the derivation walks every
directory named in `import_roots` and refuses with `WorkerBundleError::UndeclaredModule` if any
`.py` file beneath one of them is not in `inputs`.

The check is over the directory rather than over the `import` statements on purpose, because the
identity must not rest on *how* a module is reached. `importlib`, `__import__`, and a parenthesized
multi-line `from ... import (...)` all load a file that a scan of the source does not see, and each
would leave the hash sitting still while the worker's behavior changed. What an author controls is
which directories the manifest claims, so that is what is checked — and dynamic imports need no
review rule, because the module they reach is declared like any other.

**The rule this places on the worker's authors:** every module the worker loads lives beneath a
declared import root, and every `.py` file there is part of the bundle whether or not anything
imports it. A file inside the package that nothing loads is either dead code or a dynamic import,
and declaring it costs a rebuilt cache while omitting it costs a wrong one. Delete it or declare it.

Python outside every import root needs no declaration — `worker/tests/` is beside the package, not
inside it. Neither does third-party code: its identity comes from `worker/requirements.lock` and
the runtime ABI, and requiring `torch` in the input list would ask the manifest to declare something
the worker does not own.

### The declared runtime is checked too

`python` is the one declared bundle input with no file behind it, so the walk cannot reach it.
`WorkerBundle::verified_hash` therefore runs the interpreter at `worker/.venv/bin/python` and asks
what it is, refusing with `WorkerBundleError::RuntimeIdentityMismatch` when the answer differs from
the manifest. Without that, carrying the same bundle to another interpreter patch version or
platform keeps its identity while the compiled wheels it loads change — two different renders under
one cache key, which ADR-0001 §22 rules out by forbidding a human-managed marker.

**Which interpreter is asked is not an argument.** `verified_hash` takes none, and resolves
`WORKER_INTERPRETER_PATH` beneath the bundle's own root. A caller that chose the interpreter would
choose the answer: pointing the check at a system Python that happens to match the manifest passes
it while the environment that actually runs the worker is never consulted. The consequence for an
operator is the section below — the qualified environment is *attached* at that fixed path rather
than named to the tool.

The probe reads the wheel tags from `packaging`, which `requirements.lock` pins, rather than from
tag rules restated in Rust: `packaging` is the library `pip` resolves wheels with, and a
reimplementation would disagree with the environment it is meant to describe. A consequence worth
stating: **the identity cannot be read from an environment that has not been restored.** That is
the correct failure — an unrestored `worker/.venv` cannot witness the runtime the manifest claims.

**`platform_tag` records the tag carrying the ABI version, not the one `packaging` offers first.**
On Linux `platform_tags()` opens with the bare `linux_x86_64` — the tag of a wheel built for this
machine and portable nowhere — and only then yields `manylinux_2_39_x86_64` and its predecessors.
The bare tag is the same string on glibc 2.31 and glibc 2.39, so recording it would let two
environments that load different compiled wheels share one bundle identity, which is exactly what
this check exists to prevent. ADR-0001 §12.5 hashes *platform ABI* identity and
[`REFERENCE-ENVIRONMENT.md`](REFERENCE-ENVIRONMENT.md) records the reference machine's glibc among
what is qualified, so the probe skips the bare tag and records the first `manylinux_*` or
`musllinux_*` behind it.

**Which interpreter the probe runs is the attached one, not the one it resolves to.** A virtualenv's
`bin/python` is a symlink chain ending at the base interpreter, and
[Attach it at the fixed interpreter path](#attach-it-at-the-fixed-interpreter-path) makes
`worker/.venv` a link as well. Following either one lands on the system Python, whose `sys.prefix`
is `/usr` and which has none of the locked distributions — so the check would read an environment
nobody restored while reporting nothing unusual. `executable_in_place` in
`crates/study-tts-runtime/src/tools.rs` is what keeps the declared path, and names this section in
return.

### The installed environment is checked against the lock

Everything in this section and the two after it is a **precondition** on returning a bundle
identity, not an input to it. ADR-0001 §12.5 names the hash's inputs exhaustively and none of
these is among them; the digest is bit-for-bit what §12.5 specifies whether or not the check runs.
It is authorized by
[`ADR-0001-D004`](../adr/deviations/ADR-0001-D004-worker-environment-lock-verification.md), which
names `crates/study-tts-runtime/src/worker_environment.rs` in return and records what the check
costs, and which the project owner approved on 2026-08-29.

The ABI answers which wheels *could* load, not which are there, and that is a different question.
`requirements.lock` reaches the bundle identity as **bytes**, so hashing it proved what the file says
and nothing about the environment it describes. A `torch` upgraded in place left every declared input
byte-identical and every cache key where it was, while the audio changed — which is the silent cache
poisoning the hash exists to prevent, arriving through the one input the hash could not witness.

So the same probe also reports every installed distribution, and `verified_hash` compares that
against the lock before returning a hash:

| The lock says | The refusal when the environment disagrees |
|---|---|
| a pin | `EnvironmentMismatch::Absent`, or `::Version` when another version is installed |
| a governed source tree | `EnvironmentMismatch::FromIndex` when no PEP 610 record exists at all, `::WithoutRecordedRevision` when one exists but records a path rather than a revision, `::FromAnotherRevision` when it records a different one |

Names are canonicalized on both sides — `canonicalize_name` in
`crates/study-tts-runtime/src/runtime_probe.py` and `canonicalize_distribution_name` in
`crates/study-tts-runtime/src/worker_environment.rs`, which names this section in return — because a lock and a `pip freeze` routinely spell `hf-xet` and `hf_xet`
differently and they are one distribution. That mapping is many-to-one, so the probe reports a
**list** and the comparison refuses a repeated name as `EnvironmentMismatch::AmbiguousDistribution`.
Reported as a map it would have kept whichever install was walked last and called the other absent,
which is a wrong answer rather than a refusal.

**A recorded revision, not a recorded path.** PEP 610 writes `vcs_info.commit_id` for a VCS install
and only `dir_info` for a directory install, so a directory install leaves the directory's *name* as
the only evidence — and `code-<commit>-backup` beside the real tree is a name an operator really
creates, while the tree at `code-<commit>` can hold whatever was last put in it. That is why
[Install the governed Chatterbox source explicitly](#install-the-governed-chatterbox-source-explicitly)
installs from the tree's git URL at its commit: the revision then comes from what `pip` checked out
rather than from what a directory is called.

**Distributions the lock does not name are ignored.** The qualification virtualenv also carries this
repository's pre-commit tooling, which [Regenerating the lock](#regenerating-the-lock) removes from
the lock precisely because the worker does not load it; refusing it here would contradict that step
and push an operator toward a second environment. **Provenance is checked only where the lock
declares it**, for the same kind of reason: an operator restoring from a local wheelhouse writes a
`direct_url.json` for every distribution, and refusing that would be a stricter rule than the lock
states.

### Their startup hooks are not ignored

That tolerance is affordable only because an extra install stays inert, and a `.pth` file is not
inert. Python runs every `.pth` line beginning `import` as the interpreter starts, and puts every
other line on `sys.path` **ahead of** the search that resolves `torch` — so a single file no
declared bundle input covers can change what the worker loads, in a process whose identity says
nothing about it.

So the probe also reports every `.pth` file in the interpreter's site directories together with the
distribution whose `RECORD` lists it, and `check_environment_matches_lock` refuses one the lock does
not account for:

| The hook | The refusal |
|---|---|
| no installed distribution claims it, or two do | `EnvironmentMismatch::UnownedPathHook` |
| a distribution the lock does not pin owns it | `EnvironmentMismatch::UnlockedPathHook` |

The reference machine passes this as it stands: `setuptools` is pinned and ships
`distutils-precedence.pth`, and none of the pre-commit distributions removed from the lock installs
a hook at all. The rule is therefore what keeps the shared virtualenv defensible rather than what
ends it — an extra install remains fine, and an extra install that reaches into interpreter startup
does not.

### Nor are their bytes, nor the modules `site` imports by name

A `.pth` was the first thing found that is not inert; it was not the last, and two more reach the
same process by other routes. Both are observed by
`crates/study-tts-runtime/src/runtime_probe.py`, the script `worker_environment` embeds and runs
under `python -I -S`, which names this section in return.

**`-S` is what makes the probe's answer evidence rather than a claim.** Under `-I` alone the
interpreter still runs `site.main` before the script: every `.pth` file executes and `sitecustomize`
is imported — the two hazards this probe exists to report, running first, inside the process doing
the reporting. One `.pth` line replacing `json.dumps` was enough to make the probe answer that an
environment holding a modified module and an unowned hook was clean, leaving the bundle hash it
guards unchanged. `-S` suppresses both, and the script makes every observation with the standard
library before importing anything the environment supplies.
`t4_e1_interpreter_startup_code_cannot_edit_what_the_probe_reports` runs the real probe against a
real interpreter carrying exactly that hook, and is what keeps the flag from being dropped.

`-S` also skips `site.venv`, which is what moves `sys.prefix` onto a virtual environment, so the
script repeats the prefix half of it — and only that half, because the rest of `site.main` is what
`-S` is there to stop. `packaging` stays the source of the wheel tags for the reason given above, and is
the one environment-owned import: it is imported last, after every observation is made, from a site
directory the script has checked resolves inside a prefix `site` itself would search. PEP 503
canonicalization moved out of `packaging.utils` and into the script for the same ordering reason —
it is needed while walking the distributions, which is before anything may be imported — and it is
one substitution that a pinned dependency cannot state more exactly.

**A pin is not a claim about content.** A version says which release was resolved and nothing about
what the files hold now, so a module edited in place — or a `.pth` belonging to a *locked*
distribution, whose file name and owning distribution both stay correct while its lines change —
left every version, every provenance record, and every declared input byte-identical. The probe
therefore reports both the digest of the `RECORD` claims and the result of comparing files against
them. Rust first authenticates those claims against `worker/bundle-manifest.json` through
`check_records_match_their_declarations`; only then may `check_recorded_files_are_intact` turn the
reported file comparison into a refusal. Both functions live in
`crates/study-tts-runtime/src/worker_environment.rs` and name this flow in return.
Generated wheel scripts elsewhere in the same interpreter environment are not imported by
`python -m study_tts_worker` and are not read. A non-printable or absolute path, a path escaping the
interpreter environment, or a site-package symlink escaping its distribution root is refused
before any read.
Only locked distributions are inspected: an unlocked one is tolerated precisely because the
worker does not load it, and the one part of it that is not inert is already refused by name.

**This is the part of the check that costs something.** On the reference environment it reads
1,263 MiB across 31,704 files, which is 1.50 s of the 3.4–3.6 s a whole `verified_hash` takes.
That is paid once per build rather than once per segment — the bundle identity is one input to
every cache key, derived once and reused — which is why it is affordable as written and nothing
memoizes it.
[`ADR-0001-D004`](../adr/deviations/ADR-0001-D004-worker-environment-lock-verification.md) records
the measurement, and `.github/workflows/qualification.yml` times the step on every qualification
run so a drift from it is visible.

| The fault | The refusal |
|---|---|
| a recorded file holds other bytes | `EnvironmentMismatch::ModifiedDistributionFile` |
| a recorded file is absent | `EnvironmentMismatch::MissingDistributionFile` |
| the distribution ships no `RECORD` | `EnvironmentMismatch::UnrecordedDistribution` |
| a recorded digest is not canonical URL-safe base64 SHA-256 | `EnvironmentMismatch::MalformedDistributionRecord` |
| a recorded path is non-printable or escapes its permitted root | `EnvironmentMismatch::UnsafeDistributionRecord` |

**`sitecustomize` and `usercustomize` are not `.pth` files.** `site` imports both by name as the
interpreter starts, before anything the worker declares has been read, so the hook rule never saw
them. `-I` settles one of the two and not the other: it clears `ENABLE_USER_SITE`, and `site.main`
calls `execusercustomize` only under it, so a `usercustomize` resolvable on `sys.path` never runs.
Nothing suppresses `sitecustomize`. The probe therefore reports whether each one *executes* rather
than assuming it, and one that executes must be accounted for — owned by a locked distribution's
`RECORD`, or declared in `worker/bundle-manifest.json`. Otherwise
`EnvironmentMismatch::UnaccountedStartupModule`.

**Declaring one, when the machine leaves no choice.** The reference Ubuntu environment resolves
`/etc/python3.12/sitecustomize.py` before every site directory on `sys.path`. It executes in the
worker's interpreter and belongs to no distribution. Refusing it outright would ask an operator
to edit system Python; ignoring it would leave startup code outside the identity. So manifest
layout `1.1` adds `startup_modules`, and the operator records what the machine carries:

```json
"startup_modules": [
  { "module": "sitecustomize", "digest": "<RECORD-spelled SHA-256 of the file>" }
]
```

Read the digest from the refusal's own subject with
`python3 -c "import hashlib,base64,sys;print(base64.urlsafe_b64encode(hashlib.sha256(open(sys.argv[1],'rb').read()).digest()).rstrip(b'=').decode())" /etc/python3.12/sitecustomize.py`.
The field is optional and defaults to empty, so a `1.0` manifest stays valid and declares nothing —
the same absent-is-the-default extension the lesson schema made at its own `1.1`. Declaring it puts
the file where a reviewer sees it, and changing it changes the manifest, which is itself a hashed
bundle input.

A valid lock also states the two package indexes and its wheel/source policy exactly once, as
`REQUIRED_LOCK_DIRECTIVES` in `crates/study-tts-runtime/src/worker_environment.rs` requires. Every
index-supplied pin carries exactly one SHA-256 artifact hash. The governed `chatterbox-tts` pin is
the sole exception: it carries the adjacent commit marker instead because it is installed from
that VCS revision rather than from a published artifact. An unknown or repeated directive, a
missing required directive, an unhashed or multiply hashed index pin, a malformed pin, or a
governed pin parted from its provenance is refused as `UnreadableWorkerLockfile` carrying the
exact `WorkerLockfileErrorReason`; silently skipping any of those would leave resolution outside
the identity. The refusal names the invariant and a `WorkerLockfileLocus` — the line number where
one line is at fault, the file as a whole where the invariant is no line's to keep, as when the
bytes are not UTF-8, a required directive is absent, or the governed pin is missing entirely. It
never names the line's contents, and no provenance refusal prints the recorded URL: a wrongly
regenerated lock is exactly where a local-path line appears, and
[`../governance/RIGHTS-DATA-ARTIFACT-POLICY.md`](../governance/RIGHTS-DATA-ARTIFACT-POLICY.md) keeps
the governed model root out of logs. Naming the commit to reinstall from says everything the URL
would.

### Declaring what the lock installed

**`RECORD` alone answers the wrong question.** It is the distribution's own statement of which bytes
it installed, and it is installed beside them. Comparing the tree against it therefore establishes
that the install is *self-consistent* — which an edit to a module and to the line pinning that
module's digest keeps true, in one action, leaving every fault above empty. Nothing inside the
environment can close that: the artifact hashes cannot, because
[What binds the lock to artifact bytes](#what-binds-the-lock-to-artifact-bytes) records that nothing
this build can ask the interpreter reports the artifact a distribution came from.

So the claims are stated a second time from outside the environment. Manifest layout `1.2` adds
`record_digests`, one entry per locked distribution:

```json
"record_digests": [
  { "distribution": "torch", "digest": "<RECORD-spelled SHA-256 of the claims below>" }
]
```

The digest is **not** the digest of the `RECORD` file. It is taken over the `RECORD` rows this check
actually rests on — every SHA-256-bearing row resolving inside the distribution's site-package root,
excluding the `.dist-info` directory — spelled `<path>,sha256=<digest>`, sorted, and joined with
newlines. `crates/study-tts-runtime/src/runtime_probe.py` computes exactly that and names this
section in return. The exclusion is what keeps a correct restore from reading as tampering:
`.dist-info` holds `INSTALLER`, `REQUESTED`, and `direct_url.json`, which move with the command that
installed rather than with anything the worker imports.

`worker/bundle-manifest.json` is itself a declared bundle input, so this is not merely a second copy.
Changing what the lock is allowed to have installed changes the manifest, changes the bundle hash,
and moves every cache key — where a reviewer sees it. `verified_hash` authenticates the installed
`RECORD` against this declaration before it trusts any digest from that `RECORD`.
`check_records_match_their_declarations` in
`crates/study-tts-runtime/src/worker_environment.rs` is the comparison, and
`t4_e1_an_installed_record_the_manifest_does_not_vouch_for_is_refused` pins the ordering while
`t4_e1_the_probe_reads_record_digests_from_a_real_interpreter` proves that changing a module and its
`RECORD` row together is refused.

| The disagreement | The refusal |
|---|---|
| a locked distribution has no declaration | `EnvironmentMismatch::UndeclaredDistributionRecord` |
| the declared digest is not the one reported | `EnvironmentMismatch::ModifiedDistributionRecord` |

**An omitted declaration is a refusal, never an exemption.** A layout `1.0` or `1.1` manifest still
loads and declares none, and every locked distribution is then refused — which is the field working,
not a gap in it. A manifest carried from before this layout must be regenerated on the machine it
describes, from a **freshly restored** environment, before its digests mean anything. Regenerate
from an environment you have just restored and verified, never from one that has been in use, or the
declaration records whatever drift is already there:

```text
worker/.venv/bin/python -I -S crates/study-tts-runtime/src/runtime_probe.py \
  $(sed -n 's/^\([A-Za-z0-9._-]*\)==.*/\1/p' worker/requirements.lock) > /tmp/probe.json

python3 - <<'DECLARE'
import json

with open("/tmp/probe.json", encoding="utf-8") as probe:
    reported = json.load(probe)["distributions"]
print(json.dumps(
    sorted(
        ({"distribution": entry["name"], "digest": entry["record_digest"]}
         for entry in reported if entry["record_digest"]),
        key=lambda entry: entry["distribution"],
    ),
    indent=2,
))
DECLARE
```

The probe canonicalizes the names it is given, so the lock's own spellings pass through unchanged.
Paste the printed array as `record_digests` and set `schema_version` to `1.2`. Regenerating the lock
means regenerating these too, and step 12 of
[Regenerating the lock](#regenerating-the-lock) already requires the bundle hash to be recorded
afterwards for exactly that reason.

### Reading the current identity

```text
cargo run --package study-tts-runtime --example worker-bundle-hash
```

Refuses rather than printing when the manifest and the tree disagree, and when the interpreter and
the manifest disagree. Restore the environment below first. Record the printed value in any
qualification evidence taken with this worker.

## Restoring the environment

The reference machine is the one described in [`REFERENCE-ENVIRONMENT.md`](REFERENCE-ENVIRONMENT.md)
and accepted by ADR-0002.

**Build it outside the checkout, then link it in.** `worker/.venv` is gitignored, and
`actions/checkout` cleans with `git clean -ffdx`, whose `-x` removes ignored files: an environment
restored inside the workspace is deleted at the start of the next qualification run. So it lives
beside the checkout and is attached at the fixed path the runtime resolves.
`.github/workflows/qualification.yml` carries the same path as `QUALIFIED_WORKER_VENV` and names
this section in return.

```text
QUALIFIED_WORKER_VENV=<checkout-parent>/study-tts-qualified-worker-venv
WORKER_WHEELHOUSE=$(mktemp -d /tmp/study-tts-worker-wheelhouse.XXXXXX)

python3.12 -m venv "${QUALIFIED_WORKER_VENV}"
"${QUALIFIED_WORKER_VENV}/bin/python" - <<'CHECK'
import pip, sys

assert sys.version_info[:3] == (3, 12, 3), sys.version
assert pip.__version__ == "24.0", pip.__version__
CHECK
```

Use a fresh path when creating the environment. Do not upgrade `pip`: CPython 3.12.3's pinned
`ensurepip` supplies 24.0, and changing the installer changes how the one permitted source
distribution is built. The assertions fail before acquisition when either input differs.

### Acquire the hashed artifacts, minus the governed distribution

```text
set -o pipefail
grep -v '^chatterbox-tts==' worker/requirements.lock | \
  "${QUALIFIED_WORKER_VENV}/bin/python" -m pip download --isolated \
    --require-hashes --no-deps --dest "${WORKER_WHEELHOUSE}" --requirement /dev/stdin
```

The lock itself names PyPI as the primary index and the official PyTorch CPU index as the only
extra index. `--isolated` prevents a user or machine `pip` configuration from adding another one;
`--require-hashes` makes an index name insufficient without the selected artifact bytes. Every
artifact is a wheel except `s3tokenizer==0.1.7`, for which PyPI publishes only an sdist. That one
exception is declared by `--no-binary=s3tokenizer`; the default remains wheel-only.

`--no-deps` is not optional. The lock is the complete worker-loaded set; letting `pip` resolve
transitively would allow a dependency the lock does not name, and the bundle hash would then
describe an environment the machine does not have. `s3tokenizer` declares `pre-commit` as an
install dependency even though it is development tooling and is not imported at runtime. Its
packages are deliberately excluded from the synthesis identity, as described under
[Regenerating the lock](#regenerating-the-lock).

**Excluding `chatterbox-tts` from this command is not optional either, and the reason is not
obvious.** It is in `worker/requirements.lock` as an exact pin like everything else, because the
lockfile records the resolved set rather than where each distribution came from. Installing the
lockfile whole would let the configured index satisfy that pin — and the governed install that
follows would then find the requirement *already satisfied at the same version* and do nothing at
all. The environment would look correct, the version would match the lock, and the code running
would be the index's rather than the governed tree's, with nothing in the result saying so. Removing
the line leaves nothing to be already satisfied by.

### Install only from the verified wheelhouse

```text
sed -n '/^--/p;/^setuptools==/p' worker/requirements.lock | \
  "${QUALIFIED_WORKER_VENV}/bin/python" -m pip install --isolated --no-index \
    --find-links "${WORKER_WHEELHOUSE}" --require-hashes --no-deps \
    --force-reinstall --requirement /dev/stdin

grep -v '^chatterbox-tts==' worker/requirements.lock | \
  "${QUALIFIED_WORKER_VENV}/bin/python" -m pip install --isolated --no-index \
    --find-links "${WORKER_WHEELHOUSE}" --require-hashes --no-deps \
    --force-reinstall --no-build-isolation --requirement /dev/stdin
```

The first command installs the hashed `setuptools==78.1.0` wheel before the second builds the sole
sdist; `--no-build-isolation` prevents that build from resolving an unrecorded build environment.
The second command force-reinstalls every pin, so reusing an otherwise clean destination cannot
leave stale same-version bytes in place. `--no-index` makes this phase offline and proves the
wheelhouse acquired above is sufficient.

### Install the governed Chatterbox source explicitly

`chatterbox-tts` resolves from the governed local source tree rather than from an index, per
[`../governance/RIGHTS-DATA-ARTIFACT-POLICY.md`](../governance/RIGHTS-DATA-ARTIFACT-POLICY.md).

```text
"${QUALIFIED_WORKER_VENV}/bin/python" -m pip install --isolated --no-deps \
  --no-build-isolation --force-reinstall \
  "git+file://<governed-model-root>/chatterbox/code-<commit>@<commit>"
```

Model code and weights never enter Git, CI, fixtures, or logs. The commit the current lock was
resolved against is recorded as a comment beside the `chatterbox-tts` line in
`worker/requirements.lock`, and appears **twice** above: once as the directory holding the governed
clone and once as the revision to check out of it.

**Installing the directory itself is not equivalent, and the difference is the whole point.** `pip`
records where a distribution came from in PEP 610's `direct_url.json`, but for a directory install
it records only `dir_info` — a path, with no revision in it. The directory *name* is then the sole
evidence of which revision was installed, and a name is not evidence: `code-<commit>` holds whatever
was last written into it, and `code-<commit>-backup` beside it is a directory an operator really
creates. Installing from the tree's git URL at the commit makes `pip` check that revision out and
record the `commit_id` it resolved, which is an observation rather than a label. The check below is
what reads it.

**`--force-reinstall` is not optional**, and it is the same trap
[Acquire the hashed artifacts](#acquire-the-hashed-artifacts-minus-the-governed-distribution)
warns about, arriving one step later. `pip` compares the *version*, finds `0.1.2` already satisfied,
and stops — printing nothing that looks like a failure while leaving the previous install exactly
where it was. On a clean environment the flag changes nothing; on one restored by the previous
directory-install procedure it is the difference between the command working and the command
appearing to. The wheel is built before the old install is removed, so a build failure leaves the
environment as it was.

### Verify the provenance, rather than assuming the command took

Two installs of the same version are indistinguishable by version. What distinguishes them is PEP
610: `pip` writes `direct_url.json` into the `.dist-info` of anything installed from a local path or
URL, and writes **no such file** for an index install. It records `vcs_info.commit_id` only when the
install came from a VCS, so the three answers are exactly the three failures the previous section
tabulates: no record at all is the index, a record without a revision is a directory install, and a
record naming another revision is the wrong clone.

**This is checked mechanically and is not the operator's to remember.**
[The installed environment is checked against the lock](#the-installed-environment-is-checked-against-the-lock)
applies it on every `verified_hash`, driven by the `# installed from a governed local source tree at
commit ...` comment above the `chatterbox-tts` pin — `GOVERNED_SOURCE_MARKER` in
`crates/study-tts-runtime/src/worker_environment.rs`, which names this section in return.

The comment and the pin it governs are required to stay adjacent, and the lock is refused as
unreadable when they do not: a missing, blank-line-separated, doubled, or misattached comment
would otherwise leave the `chatterbox-tts` pin with no recorded origin, and a pin with no recorded
origin is not compared against PEP 610 at all. Regenerating the lock therefore means carrying the
comment with the pin, not merely keeping it somewhere in the file.

The command below stays because it is what an operator runs *while restoring*, when the failure is
cheapest to see and the bundle hash has not been asked for yet.

```text
"${QUALIFIED_WORKER_VENV}/bin/python" - <<'CHECK'
import importlib.metadata as metadata, json

record = metadata.distribution("chatterbox-tts").read_text("direct_url.json")
if record is None:
    raise SystemExit(
        "chatterbox-tts came from an index, not the governed tree; uninstall it and "
        "reinstall from git+file://<governed-model-root>/chatterbox/code-<commit>@<commit>"
    )
commit = json.loads(record).get("vcs_info", {}).get("commit_id")
if commit is None:
    raise SystemExit(
        "chatterbox-tts was installed from a path, which records no revision; uninstall "
        "it and reinstall from "
        "git+file://<governed-model-root>/chatterbox/code-<commit>@<commit>"
    )
print(commit)
CHECK
```

The printed `commit_id` must be the commit recorded beside the `chatterbox-tts` line in
`worker/requirements.lock`. A mismatch means the environment was restored from a different revision
of the model code than the lock describes, which is a synthesis-key input differing from its own
record. The URL inside the record is deliberately not printed, and the bundle probe does not report
it at all: it names the governed model root, which
[`../governance/RIGHTS-DATA-ARTIFACT-POLICY.md`](../governance/RIGHTS-DATA-ARTIFACT-POLICY.md) keeps
out of logs.

### Attach it at the fixed interpreter path

```text
if [ -e worker/.venv ] && [ ! -L worker/.venv ]; then
  echo "worker/.venv is a real directory, not a link; move or remove it first" >&2
  exit 1
fi
ln -sfnT "${QUALIFIED_WORKER_VENV}" worker/.venv
worker/.venv/bin/python -c 'import sys; print(sys.version)'
```

**Both halves of the first two lines are load-bearing, and the failure they prevent is silent.**
`ln -sfn` replaces an existing *link* but not an existing *directory*: given a real `worker/.venv` it
creates `worker/.venv/<basename of QUALIFIED_WORKER_VENV>` inside it, exits `0`, and leaves
`worker/.venv/bin/python` pointing at whatever interpreter was already there. The version line below
it then prints that interpreter's version, the operator reads a success, and every later check —
including the bundle identity — describes an environment nobody qualified. `-T` refuses to treat the
destination as a directory, so the same case becomes `ln: worker/.venv: cannot overwrite directory`.
The guard above it is what turns that into a sentence naming the remedy, and it is the same shape
`.github/workflows/qualification.yml` uses, which already attaches with `ln -sfnT`.

`.gitignore` carries `/worker/.venv` without a trailing slash, and the missing slash is the point: a
trailing one matches a directory only, so the link this command creates was reported as untracked
and could have been committed — putting a machine-local absolute path into Git, in a repository
whose whole restore procedure exists to keep the environment out of it. The pattern as written
covers both shapes, so an environment built in place is ignored too.

**Moving an existing environment is not a `mv`.** A virtualenv is not relocatable: every console
script in `bin/` carries an absolute shebang, `activate` exports an absolute `VIRTUAL_ENV`, and both
still name the old path afterwards. `bin/python` is a symlink to the system interpreter and
`sys.prefix` is computed from the invoking path, so the interpreter and the bundle probe keep
working while `pip` and every other entry point break — a failure that surfaces later and elsewhere.
Rewrite the old prefix to the new one across `bin/` and `pyvenv.cfg`, or rebuild from
[Restoring the environment](#restoring-the-environment) and reinstall the governed source.

`worker/.venv` is the only interpreter path the runtime looks at, and it is not configurable — see
[The declared runtime is checked too](#the-declared-runtime-is-checked-too). The link is re-made by
the qualification workflow after every checkout, because the clean that removes it is the same clean
that keeps the workspace honest.

## Regenerating the lock

Run on the reference machine, with network access, from a clean interpreter. A change to an index
or artifact policy changes the lock, its parser, this procedure, and the worker identity version
together.

1. Create a scratch environment: `python3.12 -m venv /tmp/worker-lock`.
2. Assert CPython 3.12.3 and `pip==24.0` as in the restore procedure; do not upgrade the installer.
3. Install only the direct dependencies declared in `worker/pyproject.toml`, using the four
   resolution directives recorded at the top of `requirements.lock`.
4. Install the governed `chatterbox-tts` source tree with `--isolated --no-deps
   --no-build-isolation --force-reinstall`.
5. Freeze: `/tmp/worker-lock/bin/python -m pip freeze`.
6. Replace the `chatterbox-tts @ file://...` line with `chatterbox-tts==<version>` and a comment
   recording the commit, so the lock never carries a machine-local path.
7. Remove the development-only distributions listed below, which share the qualification
   virtualenv but are not loaded by the worker: `cfgv`, `distlib`, `identify`, `nodeenv`,
   `pre_commit`, `python-discovery`, `virtualenv`.
8. Keep the four resolution directives at the top of the file, byte for byte:
   `--index-url https://pypi.org/simple`, `--extra-index-url https://download.pytorch.org/whl/cpu`,
   `--only-binary=:all:`, and `--no-binary=s3tokenizer`. They are `REQUIRED_LOCK_DIRECTIVES` in
   `crates/study-tts-runtime/src/worker_environment.rs`, which names this section in return; the parser
   refuses a lock missing one, repeating one, or carrying a fifth.
9. For every remaining index pin, use `pip download --isolated --no-deps` with those directives to
   acquire the artifact selected on the reference ABI, then run `pip hash <artifact>` and put that
   one SHA-256 on the pin. The governed pin receives no artifact hash.
10. Test the candidate lock by acquiring it again with `--require-hashes` into an empty
    wheelhouse, then restoring that wheelhouse offline into a fresh environment with the commands
    above. This catches a hash copied to the wrong pin, a missing artifact, and an undeclared build
    dependency.
11. Keep the explanatory header. It is what tells the next reader which limits still apply.
12. Regenerate the bundle hash and record it in the interface-change record for the story that
    caused the change, because every cache key moves with it.

Step 7 is the step most likely to be skipped, and skipping it is not cosmetic: a pre-commit hook
upgrade would otherwise change the worker-bundle hash and invalidate every cache entry in the
project for a reason that has nothing to do with speech.

### What binds the lock to artifact bytes

A version pin names which release, never which bytes: an index may serve different artifacts for
one version across ABIs, and a compromised or re-uploaded artifact keeps the version it claims. So
every index-supplied pin carries the SHA-256 of the artifact resolved for this ABI, and
`pip install --require-hashes` refuses anything else.

Three things are checked together, and each closes what the others leave open:

- **The directives** decide where a name may resolve from and whether a wheel or a source tree is
  built. Without them the same lock resolves differently under different installer configuration
  while every pin still reads as satisfied.
- **The hashes** bind each resolved name to bytes.
- **The governed commit** does the same job for `chatterbox-tts`, which has no published artifact
  to hash; PEP 610 provenance is checked before a bundle identity is returned.

`parse_lockfile` in `crates/study-tts-runtime/src/worker_environment.rs` refuses the lock when any of
the three is missing, malformed, or duplicated, and
`t4_e1_every_index_pin_requires_one_artifact_hash` and
`t4_e1_the_lock_records_its_package_sources_and_artifact_kinds` are what keep that refusal real,
and `t4_e1_a_lockfile_fault_no_line_carries_names_no_line` is what keeps a whole-file refusal from
sending an operator to a line the lock does not have.

What the hashes do **not** do is prove what is installed. Nothing this build can ask the
interpreter reports the artifact hash a distribution came from, so the parser validates each hash
and drops it. Their force is that they are part of the lock's bytes, and the lock's bytes are a
worker-bundle hash input: editing one moves every cache key.

That gap is why installed bytes are authenticated against a separate declaration rather than against
the lock. [Declaring what the lock installed](#declaring-what-the-lock-installed) records what each
locked distribution's `RECORD` must claim in `worker/bundle-manifest.json`, which is the same kind of
force — a hashed bundle input — applied to a question the artifact hashes cannot reach from here.

## Offline behavior

`worker/launcher.json` carries the configuration ADR-0001 §14 requires: `HF_HUB_OFFLINE=1`,
`TRANSFORMERS_OFFLINE=1`, `HF_HUB_DISABLE_PROGRESS_BARS=1`, and `local_files_only`. It is a declared
bundle input, so changing it changes every cache key — which is correct, because a worker that
resolves models differently is a different worker.

**Configuration is not the boundary; applying it is.** `huggingface_hub` and `transformers` decide
whether to reach a network by reading environment variables as their modules load, and neither will
ever read `worker/launcher.json`. So `study_tts_worker.worker._apply_offline_environment` puts them
into `os.environ` before any backend import. `main` first loads the launcher, then applies its
offline environment and reserves the protocol descriptor; a backend must be imported after those
startup boundaries rather than at module scope.

Which of them are mandatory is Rust's-side-of-the-mirror in Python:
`REQUIRED_OFFLINE_ENVIRONMENT` in `worker/study_tts_worker/worker.py` names `HF_HUB_OFFLINE` and
`TRANSFORMERS_OFFLINE`, and this section is what it names in return. A launcher that omits either,
sets either to anything but `1`, or turns `local_files_only` off stops the worker at startup instead
of being corrected: a default filled in here would hide the disagreement, and this build has no way
to publish audio under a cache key claiming a fetch did not happen. `HF_HUB_DISABLE_PROGRESS_BARS`
is named by `OPTIONAL_OFFLINE_ENVIRONMENT` beside it: applied like the others and not required, a
progress bar being noise rather than a fetch.

### The launcher is read closed

Those two tuples are also an **allowlist**, and that is the part worth stating rather than inferring.
`_apply_offline_environment` writes into the environment a speech backend imports from one statement
later, so what it is willing to write matters as much as what the launcher asks for. It used to copy
`offline_environment` entry by entry, which made `worker/launcher.json` a place to set `PYTHONPATH`
for that import — in a file that is a declared bundle input and therefore reads as governed. The
loop now runs over the two tuples rather than over the file, so a variable outside them cannot be
applied however it got into the launcher.

The parse is closed for the same reason, and closes it earlier. `LAUNCHER_SHAPE` in
`worker/study_tts_worker/worker.py` describes the launcher's complete shape — `schema_version`,
`device`, `threads`, `offline_environment`, `local_files_only`, `model_root_environment_variable`,
and nothing else — using the object checker in `worker/study_tts_worker/protocol.py`, so a field this
build does not describe is a refusal at startup rather than a value it silently ignores. Inside
`offline_environment` the required and optional names are the two tuples above, so the
unknown-field rule *is* the allowlist. `LAUNCHER_SCHEMA_VERSION` is `1.0` and is refused rather than
guessed at, like every other versioned record here, and it names this section in return. A string
version is checked before the current layout, because a future layout may add fields this build
cannot interpret; reporting one of them as merely unknown would hide the unsupported version.
Missing, unreadable, non-UTF-8, and malformed launcher files stop as startup errors rather than
escaping as unrelated filesystem or JSON exceptions.

Both halves exist on purpose. The parse is where an operator gets a legible refusal naming the field;
the loop is what holds if a later edit reaches `_apply_offline_environment` with a launcher that came
from somewhere else. `LauncherShapeTests` in `worker/tests/test_worker.py` drives each half.

Five tests hold the startup boundaries apart, because each proves something the others cannot:

| Test | What it proves |
|---|---|
| `OfflineEnvironmentTests.test_the_offline_environment_is_applied_before_frames_are_served` | The variables are in a **real worker process**, reported on stderr before the first frame is read. |
| `OfflineEnvironmentTests.test_a_launcher_that_permits_fetching_stops_the_worker` | A launcher that permits fetching is refused rather than tolerated. |
| `LauncherShapeTests.test_a_future_launcher_is_refused_by_version_before_its_fields` | An unsupported layout is reported before fields only that layout understands. |
| `LauncherShapeTests.test_an_unreadable_launcher_stops_as_a_startup_error` | Missing and malformed launcher files remain startup errors. |
| `t4_e1_pr_suite_performs_no_model_download` | The checked-in launcher still carries the values, so editing it without deciding to fails. |

The first four are in `worker/tests/test_worker.py`. The first uses a subprocess because only a
real worker process can prove its startup environment and stderr ordering; the other launcher
cases are unit tests at the owning boundary.

## What runs where

| Check | Where | Why |
|---|---|---|
| `python3 -m compileall worker/study_tts_worker` | `.github/workflows/ci.yml`, `lint` | A syntax error in a declared input would change the bundle hash and still ship. |
| `python3 -m unittest discover --start-directory worker/tests` | `.github/workflows/ci.yml`, `lint` | The frame reader is the worker's untrusted boundary: it must bound a frame before allocating it and refuse an unknown field at any depth. |
| Workspace suite, no network namespace egress | `.github/workflows/ci.yml`, `test` | Proves the render path needs no model. |
| Attach the qualified environment, bundle identity, full suite on real FFmpeg | `.github/workflows/qualification.yml` | Needs the qualified machine. The environment is restored once by hand and linked in per run; the workflow refuses rather than builds one. |
| Real-model measurements | Operator, by hand, per `scripts/qualification/README.md` | Their arguments name governed roots that must not appear in a workflow file. |
