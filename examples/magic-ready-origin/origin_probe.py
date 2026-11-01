#!/usr/bin/env python3
"""Inspect retained Origin warnings and run frozen actual-archive controls in private scratch."""
import gzip
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
sys.dont_write_bytecode = True
sys.path.insert(0, 'simon-game')
from native import action_root, frozen_files, load, regular, sha, tool_source

MARKER = '\n#[cfg(test)]\n#[path = "game_loop/origin_offer_regression.rs"]\nmod origin_offer_regression;\n'


def observed(report, files):
    store = Path(report['store']).resolve(strict=True)
    if not store.is_relative_to(action_root()):
        raise ValueError('verification store must belong to this run')
    run = report['run']
    if not isinstance(run, str) or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]*', run):
        raise ValueError('invalid run identity')
    _, source_cli, env = tool_source(files)
    diagnostics = Path(source_cli).parent / 'manabrew' / 'diagnostics.py'
    spec = importlib.util.spec_from_file_location('simon_frozen_diagnostics', diagnostics)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    root = store / 'runs' / run / 'attempts'
    found = []
    for attempt in sorted(root.iterdir()):
        if attempt.is_symlink() or not attempt.is_dir() or not re.fullmatch(r'[A-Za-z0-9_-]+', attempt.name):
            raise ValueError('unsafe native attempt')
        compressed = regular(attempt / 'trace.jsonl.gz')
        with gzip.GzipFile(fileobj=io.BytesIO(compressed)) as stream:
            events = [json.loads(line) for line in stream if line.strip()]
        for warning in module.trace_diagnostics(events)['warnings']:
            action = warning.get('action') or {}
            if (action.get('cardName') == 'Origin of Metalbending'
                    and warning.get('category') == 'other_insufficient_context'
                    and action.get('paymentAttempted') is False
                    and action.get('explicitCancel') is False):
                found.append({'attempt': attempt.name, 'seq': warning['seq'],
                              'trace_sha256': sha(compressed), 'category': warning['category']})
    return found


def copy_tracked_to_scratch(scratch):
    target = scratch / 'source'
    target.mkdir()
    paths = subprocess.check_output(['git', 'ls-files', '-z']).split(b'\0')
    for raw in paths:
        if not raw:
            continue
        name = os.fsdecode(raw)
        path = Path(name)
        if path.is_absolute() or '..' in path.parts:
            raise ValueError('unsafe source path')
        src, dst = Path.cwd() / path, target / path
        dst.parent.mkdir(parents=True, exist_ok=True)
        if src.is_symlink():
            dst.symlink_to(os.readlink(src))
        elif src.is_file():
            shutil.copy2(src, dst)
        else:
            raise ValueError('tracked source file missing: ' + name)
    return target


def classify_controls(exit_code, output):
    """Accept only all-pass or the one known failing no-target control."""
    if exit_code == 0 and '3 passed; 0 failed' in output:
        return 'passed'
    if (exit_code != 0 and '2 passed; 1 failed' in output
            and 'archived_origin_not_offered_without_any_legal_mode ... FAILED' in output
            and 'archived_origin_offered_when_artifact_target_is_available ... ok' in output
            and 'archived_boros_charm_offered_when_non_target_mode_is_available ... ok' in output):
        return 'reproduced'
    raise ValueError('origin control compile or unexpected test failure; inspect scratch log')


def recommendation(warnings, control):
    return 'investigate' if warnings and control == 'reproduced' else 'unresolved'


def controls(files):
    scratch = Path(tempfile.mkdtemp(prefix='origin-control-', dir=os.environ['SIMON_SCRATCH']))
    source = copy_tracked_to_scratch(scratch)
    module = source / 'manabrew-rs/crates/manabrew-engine/src/game_loop.rs'
    text = module.read_text()
    if 'mod origin_offer_regression;' in text:
        raise ValueError('unexpected existing control injection')
    module.write_text(text + MARKER)
    dest = source / 'manabrew-rs/crates/manabrew-engine/src/game_loop/origin_offer_regression.rs'
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(regular('simon-four/origin_offer_regression.rs'))
    receipt = json.loads(files['store/runtime.json'])
    archive = Path(receipt['archive']).resolve(strict=True)
    if sha(regular(archive)) != receipt['identity']['archive_sha256']:
        raise ValueError('archived card set changed')
    env = dict(os.environ, SIMON_MAGIC_ARCHIVE=str(archive), CARGO_TARGET_DIR=str(scratch / 'target'),
               CARGO_NET_OFFLINE='true', DYLD_SHARED_REGION='avoid')
    proc = subprocess.run(['cargo', 'test', '--jobs', '1', '-p', 'manabrew-engine', '--lib',
                           'origin_offer_regression', '--', '--nocapture'], cwd=source, env=env,
                          text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=1700)
    (scratch / 'origin-controls.log').write_text(proc.stdout)
    if sha(regular(archive)) != receipt['identity']['archive_sha256']:
        raise ValueError('archive changed during controls')
    frozen_files()
    return classify_controls(proc.returncode, proc.stdout)


def main():
    if sys.argv[1:] not in (['inspect'], ['test']):
        raise SystemExit('use exactly inspect or test')
    _, files = frozen_files()
    tree = subprocess.check_output(['git', 'rev-parse', 'HEAD:manabrew-rs'], text=True).strip()
    warnings = observed(load('verification.json'), files) if sys.argv[1:] == ['inspect'] else []
    result = {'source_tree': tree, 'origin_warnings': warnings, 'control': controls(files)}
    result['recommendation'] = recommendation(warnings, result['control'])
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    main()
