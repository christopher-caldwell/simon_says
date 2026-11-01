#!/usr/bin/env python3
"""Collect one declared native diagnostic in action-owned storage; never replay."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sqlite3
import subprocess
import tempfile
from native import action_root, call, frozen_files, load, regular, sha, tool_source, verified_runtime

DIAGNOSTIC = 'Unrecognized Count expression, returning 0 for: Count$LeftBattlefieldThisTurn Creature.YouCtrl'


def classify(phase, quality):
    diagnostics = quality['engineDiagnostics']
    other_diagnostics = {k: v for k, v in diagnostics.items() if k != DIAGNOSTIC and v}
    other_exclusions = {k: v for k, v in quality['exclusionReasons'].items() if k != 'unsupported_engine_behavior' and v}
    if other_diagnostics or other_exclusions:
        return 'unresolved'
    if diagnostics.get(DIAGNOSTIC, 0) > 0:
        return 'issues'
    if phase == 'baseline' or quality['exclusionReasons'] or diagnostics:
        return 'unresolved'
    return 'clean'


def inventory(analysis):
    quality = analysis['quality']
    return {'exclusions': quality['exclusionReasons'], 'eligible_warnings': quality['eligibleQualityFlags'],
            'engine_diagnostics': quality['engineDiagnostics']}


def valid_denominators(denom, eligible):
    states = denom['trialStates']
    return (denom['assignedGames'] == 10 and denom['retainedAttempts'] == 10
            and denom['eligibleGames'] == eligible and states.get('accepted', 0) == eligible
            and states.get('excluded', 0) == 10 - eligible and not (set(states) - {'accepted', 'excluded'}))


def attribution(rows, original, name, quality, kind):
    """Bind each claimed targeted exclusion to its own retained stderr."""
    attempts_root = original / 'runs' / name / 'attempts'
    if len(rows) != 10:
        raise ValueError('native database does not contain exactly ten attempts')
    matches = []
    ids, ordinals = set(), set()
    target = DIAGNOSTIC.encode()
    total_hits = 0
    excluded = 0
    for attempt_id, ordinal, state, eligible, directory, reasons_json in rows:
        if attempt_id in ids or ordinal in ordinals or not isinstance(ordinal, int):
            raise ValueError('duplicate or invalid attempt identity')
        ids.add(attempt_id); ordinals.add(ordinal)
        reason = json.loads(reasons_json)
        if not isinstance(reason, list):
            raise ValueError('invalid native exclusion reasons')
        if state == 'excluded' and eligible == 0:
            excluded += 1
        elif state != 'accepted' or eligible != 1:
            raise ValueError('native database has a pending or inconsistent trial')
        expected = attempts_root / attempt_id
        path = Path(directory)
        if path.is_symlink() or path.resolve(strict=True) != expected.resolve(strict=True) or expected.is_symlink():
            raise ValueError('attempt path mismatch')
        stderr = regular(expected / 'stderr.log')
        hits = stderr.count(target)
        total_hits += hits
        if hits:
            if state != 'excluded' or 'unsupported_engine_behavior' not in reason:
                raise ValueError('target diagnostic is not a recorded excluded attempt')
            matches.append({'attempt': attempt_id, 'ordinal': ordinal, 'stderr_sha256': sha(stderr)})
    if total_hits != quality['engineDiagnostics'].get(DIAGNOSTIC, 0):
        raise ValueError('aggregate diagnostic count does not match individual attempt logs')
    if kind == 'issues' and (not matches or len(matches) != excluded):
        raise ValueError('not all targeted exclusions are attributed to the observed diagnostic')
    return matches


def require_checkpointed_sqlite(mirror):
    wal = mirror / 'native.sqlite-wal'
    if wal.exists() and wal.stat().st_size:
        raise ValueError('cannot attribute evidence with uncheckpointed SQLite WAL')


def verify_game(report, cli, env):
    original = Path(report['store']).resolve(strict=True)
    if not original.is_relative_to(action_root()):
        raise ValueError('game store must belong to this run')
    name = report['run']
    if not isinstance(name, str) or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]*', name):
        raise ValueError('invalid native run identity')
    scratch = Path(tempfile.mkdtemp(prefix='game-verify-', dir=os.environ['SIMON_SCRATCH']))
    mirror = scratch / 'store'
    shutil.copytree(original, mirror)
    originals = original / 'runs' / name / 'attempts'
    copies = mirror / 'runs' / name / 'attempts'
    for source in originals.iterdir():
        if source.is_symlink() or not source.is_dir():
            raise ValueError('unsafe original attempt')
        dest = copies / source.name
        if dest.is_symlink() or not dest.is_dir():
            raise ValueError('unsafe copied attempt')
        shutil.rmtree(dest)
        dest.symlink_to(source.resolve(strict=True), target_is_directory=True)
    result = call(cli, env, mirror, 'verify', '--run', name)
    analysis = load(original / 'runs' / name / 'analysis.json')
    denom = analysis['denominators']
    if (result.get('run') != name or result.get('verified') != 10 or result.get('verifiedAttempts') != 10
        or result.get('retainedAttempts') != 10 or result.get('eligibleGames') != report['eligible']
        or result.get('rulesCertified') is not False or analysis['state'] != 'complete'
        or analysis['run'] != name or not valid_denominators(denom, report['eligible'])
        or analysis['runtimeIdentity'] != report['runtime_id']
        or inventory(analysis) != report['issues'] or classify(report['phase'], analysis['quality']) != report['kind']):
        raise ValueError('native result, report, issue inventory or verified denominators do not match')
    # Query only the action-owned mirror. immutable=1 avoids SQLite creating
    # auxiliary files; reject a nonempty WAL because immutable ignores it.
    require_checkpointed_sqlite(mirror)
    db = mirror / 'native.sqlite'
    with sqlite3.connect(db.resolve().as_uri() + '?mode=ro&immutable=1', uri=True) as conn:
        rows = conn.execute('SELECT attempt.id, trial.ordinal, trial.state, attempt.eligible, attempt.directory, attempt.reasons_json FROM attempt JOIN trial ON attempt.trial_id=trial.id WHERE trial.run_id=? ORDER BY trial.ordinal', (name,)).fetchall()
    matched = attribution(rows, original, name, analysis['quality'], report['kind'])
    if 'issue_attempts' in report and report['issue_attempts'] != matched:
        raise ValueError('reported per-attempt attribution differs from retained logs')
    return result, matched


def collect():
    packet = load(os.environ['SIMON_PACKET'])
    if packet['stage'] != 'simulate' or packet['gate'] != 'worker':
        raise ValueError('only simulation Worker may collect')
    root, files = frozen_files()
    source, cli, env = tool_source(files)
    record = Path('native-runtime.json')
    if record.exists():
        candidate = load(record)
        receipt = verified_runtime(candidate['receipt_path'], candidate['receipt_sha256'], candidate['source_commit'])
        subprocess.run(['git', 'merge-base', '--is-ancestor', candidate['source_commit'], packet['baseline_commit']], check=True)
        if (receipt['id'] != candidate['runtime_id'] or receipt['identity']['binary_sha256'] != candidate['binary_sha256']
                or receipt['identity']['archive_sha256'] != candidate['archive_sha256']):
            raise ValueError('candidate runtime record differs from reviewed receipt')
        phase = 'corrected'
    else:
        receipt = json.loads(files['store/runtime.json'])
        phase = 'baseline'
    store = Path(os.environ['SIMON_SCRATCH']) / 'native-store'
    store.mkdir()
    (store / 'runtime.json').write_text(json.dumps(receipt) + '\n')
    config = Path('simon-game/game-config.json')
    result = call(cli, env, store, 'run', '--config', config.resolve(), '--repo', Path.cwd())
    if (result.get('state') != 'complete' or result.get('assignedTrials') != 10
        or result.get('attempts') != 10 or result.get('rulesCertified') is not False):
        raise ValueError('native result incomplete or retried; inspect retained action scratch, do not replay')
    name = result['run']
    analysis = load(store / 'runs' / name / 'analysis.json')
    report = {'phase': phase, 'kind': classify(phase, analysis['quality']), 'run': name, 'store': str(store),
              'assigned': 10, 'attempts': 10, 'eligible': result['eligibleGames'], 'runtime_id': result['runtimeIdentity'],
              'config_sha256': sha(regular(config)), 'issues': inventory(analysis)}
    _, report['issue_attempts'] = verify_game(report, cli, env)
    Path('game.json').write_text(json.dumps(report, indent=2) + '\n')
    subprocess.run(['git', 'add', '--', 'game.json'], check=True)
    subprocess.run(['git', '-c', 'commit.gpgsign=false', 'commit', '--allow-empty', '-m', 'Record isolated native game batch'], check=True, stdout=subprocess.PIPE)
    head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
    print(json.dumps({'action_id': packet['action_id'], 'outcome': 'complete', 'report': 'Verified native game batch: ' + report['kind'],
                      'candidate_commit': head, 'inspected_commit': None}))


if __name__ == '__main__':
    collect()
