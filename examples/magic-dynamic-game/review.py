#!/usr/bin/env python3
"""Verify exact candidate evidence; choose only frozen declared route labels."""
import json
import os
from pathlib import Path
import subprocess
from game import classify, verify_game
from native import frozen_files, load, regular, sha, tool_source, verified_runtime, require_repair_transition
from probe import run_repair

p = load(os.environ['SIMON_PACKET'])
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
if p['gate'] != 'reviewer' or head != p['candidate_commit']:
    raise ValueError('candidate identity mismatch')
response = {'action_id': p['action_id'], 'outcome': 'pass', 'report': '', 'candidate_commit': None, 'inspected_commit': head}
if p['stage'] == 'simulate':
    _, files = frozen_files()
    _, cli, env = tool_source(files)
    report = load('game.json')
    if report['config_sha256'] != sha(regular('simon-game/game-config.json')):
        raise ValueError('game config does not match frozen configuration')
    if Path('native-runtime.json').exists():
        record = load('native-runtime.json')
        receipt = verified_runtime(record['receipt_path'], record['receipt_sha256'], record['source_commit'])
        subprocess.run(['git', 'merge-base', '--is-ancestor', record['source_commit'], p['baseline_commit']], check=True)
        phase = 'corrected'
    else:
        receipt = json.loads(files['store/runtime.json'])
        phase = 'baseline'
    if report['phase'] != phase or report['runtime_id'] != receipt['id']:
        raise ValueError('game used unexpected phase or runtime identity')
    verify_game(report, cli, env)
    if report['kind'] == 'unresolved':
        response.update(outcome='blocked', report='The game did not reproduce the scoped baseline issue, or has other excluded games; retain evidence and stop.')
    else:
        response.update(route=report['kind'], report='Independently verified native game evidence; route: ' + report['kind'])
elif p['stage'] == 'repair':
    changed = subprocess.check_output(['git', 'diff', '--name-only', p['baseline_commit'], head], text=True).splitlines()
    if any(x != 'repair.json' and not x.startswith('manabrew-rs/crates/manabrew-engine/') for x in changed):
        raise ValueError('candidate exceeds scoped engine repair')
    if not run_repair():
        response.update(outcome='changes_required', report='One or more frozen independent repair regressions still fail.')
    else:
        response['report'] = 'All three frozen engine regression targets passed on the exact repair candidate.'
elif p['stage'] == 'build':
    record = load('native-runtime.json')
    require_repair_transition(p, record['source_commit'])
    receipt = verified_runtime(record['receipt_path'], record['receipt_sha256'], record['source_commit'])
    if (record['runtime_id'] != receipt['id'] or record['binary_sha256'] != receipt['identity']['binary_sha256']
        or record['archive_sha256'] != receipt['identity']['archive_sha256']):
        raise ValueError('runtime record and receipt mismatch')
    _, files = frozen_files()
    source, cli, env = tool_source(files)
    spec_path = source / 'repo/sim/manabrew/engine-source.json'
    spec = load(spec_path)
    if spec.get('revision') != '44f09e473213606382168f0b08442057823b9128':
        raise ValueError('unexpected original native source pin')
    spec['revision'] = record['source_commit']
    spec_path.write_text(json.dumps(spec, indent=2) + '\n')
    # Native doctor rechecks immutable objects and all setup evidence.
    from native import call
    doctor = call(cli, env, Path(record['receipt_path']).parent, 'doctor')
    if doctor.get('ok') is not True:
        raise ValueError('native doctor did not verify the candidate runtime')
    response['report'] = 'Verified runtime receipt and immutable objects for the exact reviewed source commit.'
else:
    raise ValueError('unexpected stage')
print(json.dumps(response))
