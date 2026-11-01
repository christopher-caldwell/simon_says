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
if p['stage'] in ('supervise', 'repair'):
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
else:
    raise ValueError('unknown stage')
print(json.dumps(result))
