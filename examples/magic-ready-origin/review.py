#!/usr/bin/env python3
"""Exact-candidate mechanical review; semantic conclusions remain the Analyzer's."""
import hashlib
import json
import os
from pathlib import Path
import runpy
import subprocess
import sys

p = json.loads(Path(os.environ['SIMON_PACKET']).read_text())
readonly_roles = {'investigate': 'design.json', 'supervise': 'game.json',
                  'analyze': 'analysis.json', 'triage': 'triage.json',
                  'triage_final': 'triage-final.json'}
if p['stage'] in readonly_roles:
    current = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
    if p['gate'] != 'reviewer' or current != p['candidate_commit']:
        raise ValueError('candidate identity mismatch')
    changed = subprocess.check_output(['git', 'diff', '--name-only', p['baseline_commit'], current], text=True).splitlines()
    if any(path != readonly_roles[p['stage']] for path in changed):
        raise ValueError('non-repair role modified paths outside its declared artifact')
if p['stage'] in ('supervise', 'repair', 'build', 'simulate', 'build_origin', 'simulate_origin'):
    sys.path.insert(0, 'simon-game')
    runpy.run_path('simon-game/review.py', run_name='__main__')
    raise SystemExit(0)
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
if p['gate'] != 'reviewer' or head != p['candidate_commit']:
    raise ValueError('candidate identity mismatch')
result = {'action_id': p['action_id'], 'outcome': 'pass', 'report': '',
          'candidate_commit': None, 'inspected_commit': head}
if p['stage'] == 'investigate':
    d = json.loads(Path('design.json').read_text())
    if (d['original_question'] != p['run_context'] or d['planned_games'] != 10
            or d['config_sha256'] != hashlib.sha256(Path('simon-game/game-config.json').read_bytes()).hexdigest()):
        result.update(outcome='changes_required', report='Preserve the exact original question, ten-game bound and config SHA-256.')
    else:
        result['report'] = 'Design binds original question and exact frozen ten-game configuration.'
elif p['stage'] == 'analyze':
    sys.path.insert(0, 'simon-game')
    from game import verify_game
    from native import frozen_files, load, tool_source
    a, g = load('analysis.json'), load('game.json')
    _, files = frozen_files()
    _, cli, env = tool_source(files)
    verified, matches = verify_game(g, cli, env)
    expected = 'MAGIC-LEFT-BATTLEFIELD' if g['kind'] == 'issues' else 'none'
    if (a['original_question'] != p['run_context'] or a['verified_games'] != verified['eligibleGames']
            or a['issues'] != g['issues'] or a['repair_scope'] != expected):
        result.update(outcome='changes_required', report='Preserve the original question, verified eligible count, exact issue inventory and evidence-bound repair scope.')
    elif g['kind'] == 'unresolved':
        result.update(outcome='blocked', report='Unresolved native result: preserve evidence and stop.')
    else:
        result.update(route='issues' if g['kind'] == 'issues' else 'clean',
                      report='Independently verified native counts and full issue inventory; selected the declared route. This does not certify the Analyzer judgment or game rules.')
elif p['stage'] == 'triage':
    sys.path.insert(0, 'simon-game')
    from game import verify_game
    from native import frozen_files, load, tool_source
    a, g = load('triage.json'), load('verification.json')
    _, files = frozen_files()
    _, cli, env = tool_source(files)
    verified, matches = verify_game(g, cli, env)
    if (a['original_question'] != p['run_context'] or a['eligible_games'] != verified['eligibleGames']
            or a['issues'] != g['issues']):
        result.update(outcome='changes_required', report='Preserve the original question, independently verified eligible count, and exact complete post-repair issue inventory.')
    else:
        probe = subprocess.run(['python3', '-B', 'simon-four/origin_probe.py', 'inspect'], text=True, capture_output=True, timeout=1750)
        if probe.returncode:
            raise ValueError('origin probe failed; no repair route: ' + probe.stderr[-2000:])
        evidence = json.loads(probe.stdout)
        if a['origin_evidence'] != evidence or (a['origin_action'] == 'investigate' and evidence['recommendation'] != 'investigate'):
            result.update(outcome='changes_required', report='Preserve the exact independent origin probe result; investigate is allowed only for matching observed warning plus reproduced controls.')
        else:
            result.update(route='origin' if a['origin_action'] == 'investigate' else 'unresolved',
                          report='Verified warning inventory and independent Origin probe; live-warning causality remains unproven.')
elif p['stage'] == 'repair_origin':
    changed = subprocess.check_output(['git', 'diff', '--name-only', p['baseline_commit'], head], text=True).splitlines()
    if any(x != 'warning-repair.json' and not x.startswith('manabrew-rs/crates/manabrew-engine/') for x in changed):
        raise ValueError('Origin candidate exceeds scoped engine repair')
    probe = subprocess.run(['python3', '-B', 'simon-four/origin_probe.py', 'test'], text=True, capture_output=True, timeout=1750)
    if probe.returncode:
        raise ValueError('Origin frozen controls could not run: ' + probe.stderr[-2000:])
    if json.loads(probe.stdout)['control'] != 'passed':
        result.update(outcome='changes_required', report='All three frozen actual-archive modal offer controls must pass.')
    else:
        result['report'] = 'Three frozen actual-archive modal offer controls passed on the exact Origin repair candidate; general rules are not certified.'
elif p['stage'] == 'triage_final':
    sys.path.insert(0, 'simon-game')
    from game import verify_game
    from native import frozen_files, load, tool_source
    a, g = load('triage-final.json'), load('verification-origin.json')
    _, files = frozen_files()
    _, cli, env = tool_source(files)
    verified, matches = verify_game(g, cli, env)
    if (a['original_question'] != p['run_context'] or a['eligible_games'] != verified['eligibleGames']
            or a['issues'] != g['issues']):
        result.update(outcome='changes_required', report='Preserve the original question, independently verified eligible count and complete issue inventory.')
    else:
        result['report'] = 'Mechanically verified final counts and issue inventory; domain judgment is not certified.'
else:
    raise ValueError('unknown stage')
print(json.dumps(result))
