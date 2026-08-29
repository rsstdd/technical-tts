"""Reports what the interpreter that runs the worker is, and what is installed beside it.

Loaded by `crates/study-tts-runtime/src/worker_environment.rs` through
`include_str!` and executed with `python -I -c`, so it must stay a single
self-contained script with no imports outside the standard library and
`packaging`, which `worker/requirements.lock` pins.

`docs/operations/WORKER-ENVIRONMENT.md` states the rules this script observes in
prose and names this file in return.
"""
import base64, csv, hashlib, json, os, site, sys
from importlib.metadata import distributions
from importlib.util import find_spec
from packaging.tags import platform_tags, sys_tags
from packaging.utils import canonicalize_name
tag = next(iter(sys_tags()))
# The bare `linux_<arch>` carries no ABI version, so it is skipped in
# favour of the `manylinux`/`musllinux` tag behind it. Kept as the
# fallback rather than refused here: an environment with no detectable
# platform ABI is one the manifest comparison should reject by name.
platforms = list(platform_tags())
abi_platform = next(
    (name for name in platforms if not name.startswith('linux_')),
    platforms[0],
)
# The lock's own names, passed as arguments rather than parsed here. Rust
# owns the lockfile grammar, and reading it twice in two languages is two
# grammars that drift. It also bounds the cost below: `RECORD`
# verification reads every file it lists, and the tolerated extras --
# this repository's pre-commit tooling among them -- are not what the
# worker loads.
locked = set(sys.argv[1:])
environment_root = os.path.realpath(sys.prefix)
installed = []
owners = {}
claimed = {}
faults = []
# Kept in step with `worker_bundle.rs::validate_record_digest`; the probe
# refuses installed metadata and Serde refuses the manifest/report boundary.
record_digest_length = 43
record_digest_alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-'
record_digest_final = 'AEIMQUYcgkosw048'

def report_fault(fault):
    if not faults:
        faults.append(fault)

def digest_of(path):
# Chunked because a locked distribution ships weights-sized files, and
# reading one whole is a resident copy of it.
    hasher = hashlib.sha256()
    with open(path, 'rb') as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b''):
            hasher.update(chunk)
    return base64.urlsafe_b64encode(hasher.digest()).rstrip(b'=').decode()

for dist in distributions():
    name = dist.metadata['Name']
    if not name:
        continue
    canonical = canonicalize_name(name)
    record = dist.read_text('direct_url.json')
    source = json.loads(record) if record else {}
# A list rather than a map keyed by the canonical name: two distributions
# that canonicalize alike are a broken environment, and a map would drop
# one of them silently. Rust builds the map and refuses the collision.
    installed.append({
        'name': canonical,
        'version': dist.version,
        'recorded_source': record is not None,
        'commit': source.get('vcs_info', {}).get('commit_id'),
    })
    for entry in dist.files or ():
        if not os.path.basename(str(entry)).endswith('.pth'):
            continue
        hook = os.path.realpath(str(dist.locate_file(entry)))
# Two distributions claiming one hook file leave it unowned rather than
# attributed to whichever was walked last, so the ambiguity is refused.
        owners[hook] = None if hook in owners else canonical
# `RECORD` is the distribution's own statement of which bytes it installed,
# so comparing the tree against it detects drift from that installed
# record. Checked only for the locked distributions: an
# unlocked one is tolerated precisely because the worker does not load it,
# and its `.pth` -- the one part of it that is not inert -- is already
# refused by name.
    if canonical not in locked:
        continue
    listing = dist.read_text('RECORD')
    if listing is None:
        report_fault({'distribution': canonical, 'fault': 'unrecorded'})
        continue
    base = os.path.realpath(str(dist.locate_file('')))
    for row in csv.reader(listing.splitlines()):
        if not row:
            continue
        relative = row[0]
        recorded = row[1] if len(row) > 1 else ''
        if not relative.isprintable():
            report_fault({'distribution': canonical,
                          'fault': 'unsafe_record'})
            continue
# `RECORD` lists itself with an empty hash, and an installer may leave a
# generated file unhashed. An entry that states no digest is a file the
# distribution declined to pin, not one this check may invent a digest
# for, so it is skipped rather than reported as a fault.
        if not recorded.startswith('sha256='):
            continue
        digest = recorded[len('sha256='):]
        if (len(digest) != record_digest_length or
                any(character not in
                    record_digest_alphabet
                    for character in digest) or
                digest[-1] not in record_digest_final):
            report_fault({'distribution': canonical,
                          'fault': 'malformed_record'})
            continue
        target = os.path.abspath(os.path.join(base, relative))
        if (os.path.isabs(relative) or
                os.path.commonpath((environment_root, target)) != environment_root):
            report_fault({'distribution': canonical,
                          'fault': 'unsafe_record'})
            continue
# Wheel scripts legitimately live under the environment's `bin`
# directory and do not run inside `python -m study_tts_worker`. Do not
# read them, but do refuse a site-package path whose symlink resolves out
# of the distribution tree.
        if os.path.commonpath((base, target)) != base:
            continue
        target = os.path.realpath(target)
        if os.path.commonpath((base, target)) != base:
            report_fault({'distribution': canonical,
                          'fault': 'unsafe_record'})
            continue
        claimed[target] = canonical
        if faults:
            continue
        if not os.path.isfile(target):
            report_fault({'distribution': canonical, 'file': relative,
                          'fault': 'missing'})
            continue
        if digest_of(target) != digest:
            report_fault({'distribution': canonical, 'file': relative,
                          'fault': 'modified'})
hooks = []
for directory in site.getsitepackages():
    if not os.path.isdir(directory):
        continue
    for entry in sorted(os.listdir(directory)):
        if entry.endswith('.pth'):
            path = os.path.realpath(os.path.join(directory, entry))
            hooks.append({'file': entry, 'owner': owners.get(path)})
# `site` imports these by name as the interpreter starts, before anything
# this probe reports has been read. `-I` settles `usercustomize` -- it
# clears `ENABLE_USER_SITE`, and `site.main` calls `execusercustomize`
# only under it -- but nothing suppresses `sitecustomize`, so whether each
# one *executes* is reported rather than assumed.
startup = []
for module, executes in (
    ('sitecustomize', True),
    ('usercustomize', bool(site.ENABLE_USER_SITE)),
):
    try:
        spec = find_spec(module)
# A startup module whose own import machinery raises is reported as
# present and unowned rather than skipped: the failure says something is
# there, and silence would read as an empty environment.
    except Exception:
        startup.append({'module': module, 'executes': executes,
                        'owner': None, 'digest': None})
        continue
    if spec is None or not spec.origin or spec.origin == 'built-in':
        continue
    origin = os.path.realpath(spec.origin)
    startup.append({
        'module': module,
        'executes': executes,
        'owner': claimed.get(origin),
        'digest': digest_of(origin) if os.path.isfile(origin) else None,
    })
json.dump({
    'runtime': {
        'implementation': sys.implementation.name,
        'version': '.'.join(str(part) for part in sys.version_info[:3]),
        'abi_tag': tag.abi,
        'platform_tag': abi_platform,
    },
    'distributions': installed,
    'path_hooks': hooks,
    'integrity_faults': faults,
    'startup_modules': startup,
}, sys.stdout)
