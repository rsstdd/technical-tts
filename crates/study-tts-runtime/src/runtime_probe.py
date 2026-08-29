"""Reports what the interpreter that runs the worker is, and what is installed beside it.

Loaded by `crates/study-tts-runtime/src/worker_environment.rs` through
`include_str!` and executed with `python -I -S -c`, so it must stay a single
self-contained script. Nothing outside the standard library is imported until
every observation below has been made; `packaging`, which
`worker/requirements.lock` pins, is imported at the foot of this file and only
for the wheel tags.

`docs/operations/WORKER-ENVIRONMENT.md` states the rules this script observes in
prose and names this file in return.
"""
import base64, csv, hashlib, json, os, re, site, sys
from importlib.machinery import PathFinder
from importlib.metadata import distributions
# `-S` is what makes this probe's answer worth reading: without it `site.main`
# runs first, and every `.pth` file, `sitecustomize`, and `usercustomize` this
# script exists to report has already executed inside the process doing the
# reporting. `-I` alone does not stop them.
#
# The cost is that `site.main` also does not run, and it is `site.venv` that
# moves `sys.prefix` onto a virtual environment -- `-S` leaves it at the base
# installation, where none of the locked distributions live. So the prefix half
# of `site.venv` is repeated here, and only that half: `addsitepackages` reads
# `.pth` files and `execsitecustomize` imports one, which is what `-S` is for.
executable_directory = os.path.dirname(os.path.abspath(sys.executable))
site_prefix = os.path.dirname(executable_directory)
environment_configuration = next(
    (path for path in (os.path.join(executable_directory, 'pyvenv.cfg'),
                       os.path.join(site_prefix, 'pyvenv.cfg'))
     if os.path.isfile(path)),
    None,
)
if environment_configuration is not None:
    system_site = False
    with open(environment_configuration, encoding='utf-8') as handle:
        for line in handle:
            key, separator, value = line.partition('=')
            key = key.strip().lower()
            if separator and key == 'include-system-site-packages':
                system_site = value.strip().lower() == 'true'
# `site.getsitepackages` reads `PREFIXES`, and compares `sys.prefix` against
# `sys.base_prefix` to recognize an environment at all, so all three
# assignments are needed for it to answer about the environment rather than
# about the interpreter the environment was built from.
    sys.prefix = sys.exec_prefix = site_prefix
    site.PREFIXES = [site_prefix] + (site.PREFIXES if system_site else [])
# The directories the interpreter would put on `sys.path`, kept only where they
# resolve inside a prefix `site` itself would search. Under `-S` this list is
# the only thing added to `sys.path` at the foot of this file, so it is
# validated before it is used: a site directory symlinked out of its prefix is
# dropped rather than read, and the distributions it holds are then reported
# absent, which is a refusal rather than a silent import from somewhere else.
prefix_roots = [os.path.realpath(prefix) for prefix in site.PREFIXES if prefix]
site_directories = []
for directory in site.getsitepackages():
    resolved = os.path.realpath(directory)
    if not os.path.isdir(resolved) or resolved in site_directories:
        continue
    if any(os.path.commonpath((root, resolved)) == root
           for root in prefix_roots):
        site_directories.append(resolved)
# What the interpreter would search for a top-level module, minus anything a
# `.pth` would add to it: `-S` keeps those lines from running, and a `.pth`
# this build does not account for is already refused by name.
search_path = list(dict.fromkeys(sys.path + site_directories))
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


def canonicalize_name(name):
# The whole of PEP 503 canonicalization, and the whole of
# `packaging.utils.canonicalize_name`. Spelled out because that module cannot
# be imported until every observation here is made, and because one
# substitution is not a rule a pinned dependency can state more exactly.
    return re.sub(r'[-_.]+', '-', name).lower()


def report_fault(fault):
    if not faults:
        faults.append(fault)


def encode_digest(hasher):
    return base64.urlsafe_b64encode(hasher.digest()).rstrip(b'=').decode()


def digest_of(path):
# Chunked because a locked distribution ships weights-sized files, and
# reading one whole is a resident copy of it.
    hasher = hashlib.sha256()
    with open(path, 'rb') as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b''):
            hasher.update(chunk)
    return encode_digest(hasher)


# The lock's own names, passed as arguments rather than parsed here. Rust
# owns the lockfile grammar, and reading it twice in two languages is two
# grammars that drift. It also bounds the cost below: `RECORD`
# verification reads every file it lists, and the tolerated extras --
# this repository's pre-commit tooling among them -- are not what the
# worker loads. Canonicalized on arrival although Rust already canonicalizes,
# so the operator command in `docs/operations/WORKER-ENVIRONMENT.md` may pass
# the lock's spellings straight through.
locked = {canonicalize_name(name) for name in sys.argv[1:]}
for dist in distributions(path=search_path):
    name = dist.metadata['Name']
    if not name:
        continue
    canonical = canonicalize_name(name)
    record = dist.read_text('direct_url.json')
    source = json.loads(record) if record else {}
# A list rather than a map keyed by the canonical name: two distributions
# that canonicalize alike are a broken environment, and a map would drop
# one of them silently. Rust builds the map and refuses the collision.
    entry = {
        'name': canonical,
        'version': dist.version,
        'recorded_source': record is not None,
        'commit': source.get('vcs_info', {}).get('commit_id'),
        'record_digest': None,
    }
    installed.append(entry)
    for file in dist.files or ():
        if not os.path.basename(str(file)).endswith('.pth'):
            continue
        hook = os.path.realpath(str(dist.locate_file(file)))
# Two distributions claiming one hook file leave it unowned rather than
# attributed to whichever was walked last, so the ambiguity is refused.
        owners[hook] = None if hook in owners else canonical
# `RECORD` is the distribution's own statement of which bytes it installed, and
# is authenticated against `worker/bundle-manifest.json` rather than trusted:
# `record_digest` below pins the claims this comparison rests on, so editing a
# file and its `RECORD` together moves a digest the environment cannot reach.
# Checked only for the locked distributions: an unlocked one is tolerated
# precisely because the worker does not load it, and its `.pth` -- the one part
# of it that is not inert -- is already refused by name.
    if canonical not in locked:
        continue
    listing = dist.read_text('RECORD')
    if listing is None:
        report_fault({'distribution': canonical, 'fault': 'unrecorded'})
        continue
    base = os.path.realpath(str(dist.locate_file('')))
    verified = []
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
# The `.dist-info` directory is the installer's bookkeeping -- `INSTALLER`,
# `REQUESTED`, `direct_url.json` -- which varies with the command that
# installed rather than with what the worker imports, and holds nothing the
# interpreter loads. Every other claim this check rests on is pinned.
        if not relative.split('/', 1)[0].endswith('.dist-info'):
            verified.append(relative + ',' + recorded)
        if faults:
            continue
        if not os.path.isfile(target):
            report_fault({'distribution': canonical, 'file': relative,
                          'fault': 'missing'})
            continue
        if digest_of(target) != digest:
            report_fault({'distribution': canonical, 'file': relative,
                          'fault': 'modified'})
# The canonical form `worker/bundle-manifest.json` declares this distribution
# by, and the other end of the mirror in
# `docs/operations/WORKER-ENVIRONMENT.md` §Declaring what the lock installed
# and in `worker_bundle.rs::DeclaredDistributionRecord`, both of which name
# this script in return. Sorted so the digest states a set of claims rather
# than the order an installer happened to write them in.
    entry['record_digest'] = encode_digest(
        hashlib.sha256('\n'.join(sorted(verified)).encode()))
hooks = []
for directory in site_directories:
    for name in sorted(os.listdir(directory)):
        if name.endswith('.pth'):
            path = os.path.realpath(os.path.join(directory, name))
            hooks.append({'file': name, 'owner': owners.get(path)})
# `site` imports these by name as the interpreter starts, before anything
# this probe reports has been read. `-I` settles `usercustomize`: `site.main`
# calls `execusercustomize` only under `ENABLE_USER_SITE`, which
# `check_enableusersite` clears whenever `no_user_site` is set. The flag is
# read here rather than `site.ENABLE_USER_SITE`, which `-S` leaves unset
# because `site.main` never ran. Nothing suppresses `sitecustomize`, so whether
# each one *executes* is reported rather than assumed. Resolved through
# `PathFinder`, which searches an explicit path and imports nothing: under `-S`
# neither module has run, and finding one must not be what runs it.
startup = []
for module, executes in (
    ('sitecustomize', True),
    ('usercustomize', not sys.flags.no_user_site),
):
    try:
        spec = PathFinder.find_spec(module, search_path)
# A startup module whose own import machinery raises is reported as
# present and unowned rather than skipped: the failure says something is
# there, and silence would read as an empty environment.
    except Exception:
        startup.append({'module': module, 'executes': executes,
                        'owner': None, 'digest': None})
        continue
    if spec is None or not spec.origin:
        continue
    origin = os.path.realpath(spec.origin)
    startup.append({
        'module': module,
        'executes': executes,
        'owner': claimed.get(origin),
        'digest': digest_of(origin) if os.path.isfile(origin) else None,
    })
# Everything above is observed with the standard library alone. `packaging` is
# the one environment-owned import this probe makes, and it comes last so that
# no code the environment supplies runs before the observations about that
# environment are complete. The tags come from `packaging` rather than from tag
# rules restated here because it is the library `pip` resolves wheels with, and
# a reimplementation would disagree with the environment it describes.
#
# The serializer and the output stream are bound first. What the import defines
# cannot replace either, so the worst a tampered `packaging` can do is misreport
# the two tags below -- which the manifest comparison refuses -- rather than
# rewrite the integrity findings already gathered.
serialize, channel = json.dumps, sys.stdout.buffer
sys.path.extend(site_directories)
from packaging.tags import platform_tags, sys_tags
tag = next(iter(sys_tags()))
# The bare `linux_<arch>` carries no ABI version, so it is skipped in favour of
# the `manylinux`/`musllinux` tag behind it. Kept as the fallback rather than
# refused here: an environment with no detectable platform ABI is one the
# manifest comparison should reject by name.
platforms = list(platform_tags())
abi_platform = next(
    (name for name in platforms if not name.startswith('linux_')),
    platforms[0],
)
channel.write(serialize({
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
}).encode())
channel.flush()
