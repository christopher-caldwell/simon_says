#!/usr/bin/env python3
"""Native tooling and source identity pinned by frozen manifest and action receipts."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import tempfile


def regular(path):
    path = Path(path)
    st = path.lstat()
    if not stat.S_ISREG(st.st_mode):
        raise ValueError('not a regular file: ' + str(path))
    fd = os.open(path, os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0))
    with os.fdopen(fd, 'rb') as stream:
        if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
            raise ValueError('not a regular file: ' + str(path))
        return stream.read()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def load(path):
    return json.loads(regular(path))


def capture(root):
    root = Path(root).resolve(strict=True)
    sim = root / 'repo/sim'
    package = sim / 'manabrew'
    for directory in (root / 'repo', sim, package, root / 'store'):
        if directory.is_symlink() or not directory.is_dir():
            raise ValueError('unsafe runtime directory')
    files = {'repo/sim/playtest': regular(sim / 'playtest'), 'store/runtime.json': regular(root / 'store/runtime.json')}
    for current, dirs, names in os.walk(package, followlinks=False):
        base = Path(current)
        keep = []
        for name in dirs:
            if name in ('__pycache__', '.git', 'target'):
                continue
            path = base / name
            if path.is_symlink():
                raise ValueError('symlinked native source directory')
            keep.append(name)
        dirs[:] = keep
        for name in names:
            if name.endswith(('.pyc', '.pyo')):
                continue
            path = base / name
            files[path.relative_to(root).as_posix()] = regular(path)
    receipt = json.loads(files['store/runtime.json'])
    identity = receipt['identity']
    for key, digest_key in [('binary', 'binary_sha256'), ('archive', 'archive_sha256')]:
        path = Path(receipt[key]).resolve(strict=True)
        if not path.is_relative_to(root):
            raise ValueError('baseline runtime object escapes isolated root')
        if sha(regular(path)) != identity[digest_key]:
            raise ValueError('baseline runtime object hash mismatch')
    return files


def manifest(files):
    return {key: sha(value) for key, value in sorted(files.items())}


def frozen_files():
    config = load('simon-game/assets.json')
    root = Path(config['root'])
    files = capture(root)
    if manifest(files) != config['files']:
        raise ValueError('frozen native source or runtime receipt changed')
    return root, files


def action_root():
    return Path(os.environ['SIMON_PACKET']).resolve(strict=True).parents[1]


def verified_runtime(path, expected_hash=None, source_commit=None):
    path = Path(path).resolve(strict=True)
    if not path.is_relative_to(action_root()):
        raise ValueError('candidate runtime must belong to a prior action in this run')
    data = regular(path)
    if expected_hash is not None and sha(data) != expected_hash:
        raise ValueError('candidate runtime receipt changed')
    receipt = json.loads(data)
    if source_commit is not None and receipt['identity']['revision'] != source_commit:
        raise ValueError('candidate runtime is not built from the reviewed source commit')
    root = path.parent
    for key, digest_key in [('binary', 'binary_sha256'), ('archive', 'archive_sha256')]:
        asset = Path(receipt[key]).resolve(strict=True)
        if not asset.is_relative_to(root):
            raise ValueError('candidate runtime object escapes owned runtime')
        if sha(regular(asset)) != receipt['identity'][digest_key]:
            raise ValueError('candidate runtime object changed')
    return receipt


def tool_source(files):
    scratch = Path(os.environ['SIMON_SCRATCH'])
    root = Path(tempfile.mkdtemp(prefix='native-source-', dir=scratch))
    for name, data in files.items():
        if name.startswith('repo/sim/'):
            target = root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
    cli = root / 'repo/sim/playtest'
    cli.chmod(0o755)
    bindir = scratch / 'bin'
    bindir.mkdir(exist_ok=True)
    shim = bindir / 'ps'
    shutil.copyfile('simon-game/ps_compat.py', shim)
    shim.chmod(0o755)
    env = dict(os.environ, PATH=str(bindir) + os.pathsep + os.environ.get('PATH', ''),
               PYTHONNOUSERSITE='1', CARGO_NET_OFFLINE='true', DYLD_SHARED_REGION='avoid')
    env.pop('PYTHONPATH', None)
    env.pop('PYTHONHOME', None)
    env['PYTHONPYCACHEPREFIX'] = tempfile.mkdtemp(prefix='pycache-', dir=scratch)
    return root, cli, env


def call(cli, env, store, *args, timeout=900):
    result = subprocess.run([str(cli), '--local', str(store), *map(str, args)], env=env, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout)
    if result.returncode:
        raise RuntimeError(f'native command failed {result.returncode}: {result.stderr[-3000:]}')
    return json.loads(result.stdout)


def require_repair_transition(packet, source_commit):
    transition = packet.get('last_transition') or {}
    if (source_commit != packet['baseline_commit'] or transition.get('from_stage') != 'repair'
            or transition.get('to_stage') != 'build' or transition.get('reviewed_commit') != source_commit):
        raise ValueError('runtime must come from the exact reviewed repair checkpoint')
