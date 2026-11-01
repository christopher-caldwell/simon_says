#!/usr/bin/env python3
"""Run the bounded Origin evidence probe under controller command supervision."""
import json
import os
from pathlib import Path
import subprocess

packet = json.loads(Path(os.environ['SIMON_PACKET']).read_text())
if packet['stage'] != 'probe_origin' or packet['gate'] != 'worker':
    raise ValueError('unexpected action identity')
probe = subprocess.run(['python3', '-B', 'simon-four/origin_probe.py', 'inspect'],
                       text=True, capture_output=True, timeout=1750, check=True)
evidence = json.loads(probe.stdout)
Path('origin-evidence.json').write_text(json.dumps(evidence, indent=2, sort_keys=True) + '\n')
subprocess.run(['git', 'add', '--', 'origin-evidence.json'], check=True)
subprocess.run(['git', 'commit', '-m', 'Record bounded Origin probe evidence'], check=True, stdout=subprocess.DEVNULL)
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
print(json.dumps({'action_id': packet['action_id'], 'outcome': 'complete',
                  'report': 'Recorded the bounded Origin probe; causal attribution remains unresolved.',
                  'candidate_commit': head, 'inspected_commit': None}))
