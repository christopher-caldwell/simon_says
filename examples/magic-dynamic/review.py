#!/usr/bin/env python3
"""Review exact candidates and independently run the frozen regression."""
import json
import os
from pathlib import Path
import subprocess
from probe import run, run_repair

p = json.loads(Path(os.environ['SIMON_PACKET']).read_text())
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
if p['gate'] != 'reviewer' or head != p['candidate_commit']:
    raise ValueError('review checkout does not match candidate')
response = {'action_id': p['action_id'], 'outcome': 'pass', 'report': '',
            'candidate_commit': None, 'inspected_commit': head}
if p['stage'] == 'observe':
    kind, _ = run()
    finding = json.loads(Path('finding.json').read_text())
    if finding['kind'] != kind or finding['issue_id'] != 'MAGIC-LEFT-BATTLEFIELD':
        response.update(outcome='changes_required', report='Correct finding.json to match the independently executed engine regression.')
    else:
        response.update(report='Independently checked exact candidate and real engine regression: ' + kind, route=kind)
elif p['stage'] == 'repair':
    changed = subprocess.check_output(['git', 'diff', '--name-only', p['baseline_commit'], head], text=True).splitlines()
    if any(x != 'repair.json' and not x.startswith('manabrew-rs/crates/manabrew-engine/') for x in changed):
        raise ValueError('candidate exceeds scoped engine repair')
    if not run_repair():
        response.update(outcome='changes_required', report='The frozen real-engine regression still fails; repair it before resubmitting.')
    else:
        response['report'] = 'All three frozen engine regression targets passed on the exact repair candidate; no full game or runtime certification.'
else:
    raise ValueError('unexpected stage')
print(json.dumps(response))
