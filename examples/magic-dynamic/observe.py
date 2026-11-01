#!/usr/bin/env python3
"""Commit structured observation from an actual historical-engine regression."""
import json
import os
from pathlib import Path
import subprocess
from probe import run

p = json.loads(Path(os.environ['SIMON_PACKET']).read_text())
if p['gate'] != 'worker' or p['stage'] != 'observe':
    raise ValueError('only the observe Worker may run this action')
kind, digest = run()
report = {'kind': kind, 'issue_id': 'MAGIC-LEFT-BATTLEFIELD', 'verification_log_sha256': digest,
          'evidence': 'Previously retained native game diagnostics and isolated engine regression; not a new game.'}
Path('finding.json').write_text(json.dumps(report, indent=2) + '\n')
subprocess.run(['git', 'add', '--', 'finding.json'], check=True)
subprocess.run(['git', '-c', 'commit.gpgsign=false', 'commit', '--allow-empty', '-m', 'Record engine regression observation'], check=True, stdout=subprocess.PIPE)
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
print(json.dumps({'action_id': p['action_id'], 'outcome': 'complete', 'report': 'Recorded real engine regression result: ' + kind,
                  'candidate_commit': head, 'inspected_commit': None}))
