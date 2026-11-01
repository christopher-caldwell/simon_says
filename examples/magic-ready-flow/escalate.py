#!/usr/bin/env python3
"""Durably stop after reviewed warning triage rather than claim readiness."""
import json
import os
from pathlib import Path
p = json.loads(Path(os.environ['SIMON_PACKET']).read_text())
if p['stage'] != 'escalate' or p['gate'] != 'worker':
    raise ValueError('only escalation Worker may invoke this stop')
print(json.dumps({'action_id': p['action_id'], 'outcome': 'blocked',
                  'report': 'Post-repair warnings remain; reviewed triage is retained. A verified scope and acceptance criteria are needed before further repair.',
                  'candidate_commit': None, 'inspected_commit': None}))
