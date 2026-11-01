#!/usr/bin/env python3
"""Mechanical review with a deliberately induced missing audit field."""
import json
import os
from pathlib import Path
import subprocess
from native_tool import mirror_verify

p = json.loads(Path(os.environ['SIMON_PACKET']).read_text())
a = json.loads(Path('analysis.json').read_text())
c = json.loads(Path('collection.json').read_text())
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
if p['gate'] != 'reviewer' or head != p['candidate_commit']:
    raise ValueError('review candidate identity mismatch')
result = mirror_verify(c)
errors = []
if a['original_question'] != p.get('run_context'):
    errors.append('set original_question to packet.run_context')
if a['verified_games'] != result['eligibleGames']:
    errors.append('set verified_games to collection.eligible')
if any(issue not in a['issues'] for issue in c['issues']):
    errors.append('include each collection issue object unchanged; extra issues are permitted')
if a.get('excluded_count') != c['assigned'] - c['eligible']:
    errors.append('add integer excluded_count equal to collection.assigned minus collection.eligible')
print(json.dumps({'action_id':p['action_id'], 'outcome':'changes_required' if errors else 'pass',
                  'report': '; '.join(errors) if errors else 'Verified retained native evidence and explicit excluded count; mechanical review does not certify domain judgment.',
                  'candidate_commit':None, 'inspected_commit':head}))
