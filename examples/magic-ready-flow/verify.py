#!/usr/bin/env python3
"""Independently verify native evidence; never collect or change original store."""
import json
from pathlib import Path
import sys
sys.path.insert(0, 'simon-game')
from game import verify_game
from native import frozen_files, load, tool_source

if sys.argv[1:] not in (['verify'], ['verify-post']):
    raise SystemExit('use exactly verify or verify-post')
_, files = frozen_files()
_, cli, env = tool_source(files)
report = load('verification.json' if sys.argv[1:] == ['verify-post'] else 'game.json')
result, matches = verify_game(report, cli, env)
print(json.dumps({'verified_attempts': result['verifiedAttempts'], 'eligible': result['eligibleGames'],
                  'issue_attempts': matches, 'issues': report['issues'], 'rules_certified': result['rulesCertified']}))
