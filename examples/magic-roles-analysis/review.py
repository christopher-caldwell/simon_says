#!/usr/bin/env python3
import json, os, subprocess
from pathlib import Path
from native_tool import mirror_verify, evidence
p=json.loads(Path(os.environ['SIMON_PACKET']).read_text()); a=json.loads(Path('analysis.json').read_text()); c=json.loads(Path('collection.json').read_text())
head=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
if p['gate'] != 'reviewer' or head != p['candidate_commit']:
    raise ValueError('review does not match exact candidate')
root, run = evidence()
native=json.loads((root/'runs'/run/'analysis.json').read_text())
quality=native['quality']
issues=[{'kind':'exclusion','details':quality['exclusionReasons']},{'kind':'eligible_warning','details':quality['eligibleQualityFlags']}]
if c['issues'] != issues or c['assigned'] != 10 or c['attempts'] != 10 or c['eligible'] != 6:
    raise ValueError('frozen collection differs from original native packet')
r=mirror_verify(c)
valid=a['original_question']==p.get('run_context') and a['verified_games']==r['eligibleGames'] and a['issues']==issues
print(json.dumps({'action_id':p['action_id'],'outcome':'pass' if valid else 'changes_required','report':'Independently verified all 10 attempts, 6 eligible, 4 excluded, and complete native issue inventory; review does not certify domain judgment.' if valid else 'Correct the question, verified eligible count and full issue inventory.','candidate_commit':None,'inspected_commit':head}))
