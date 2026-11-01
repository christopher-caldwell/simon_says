#!/usr/bin/env python3
"""Run a bounded real-engine regression; unknown failures are never issues or success."""
import hashlib
import os
from pathlib import Path
import subprocess

EXPRESSION = 'Unrecognized Count expression, returning 0 for: Count$LeftBattlefieldThisTurn Creature.YouCtrl'


def classify(code, output):
    if code == 0 and 'test result: ok. 2 passed; 0 failed' in output:
        return 'clean'
    if code != 0 and 'test result: FAILED. 0 passed; 2 failed' in output and EXPRESSION in output:
        return 'issues'
    raise ValueError('regression did not produce a recognized full result; preserve logs and stop')


def run():
    scratch = Path(os.environ['SIMON_SCRATCH'])
    env = dict(os.environ, CARGO_TARGET_DIR=str(scratch / 'target'), DYLD_SHARED_REGION='avoid', CARGO_NET_OFFLINE='true')
    result = subprocess.run(['cargo', 'test', '-p', 'manabrew-engine', '--test', 'left_battlefield_this_turn', '--', '--nocapture'],
                            env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=1700)
    output = result.stdout
    (scratch / 'engine-regression.log').write_text(output)
    kind = classify(result.returncode, output)
    return kind, hashlib.sha256(output.encode()).hexdigest()


def run_repair():
    """Require three immutable independent regression targets on exact candidate."""
    scratch = Path(os.environ['SIMON_SCRATCH'])
    env = dict(os.environ, CARGO_TARGET_DIR=str(scratch / 'target'), DYLD_SHARED_REGION='avoid', CARGO_NET_OFFLINE='true')
    result = subprocess.run(['cargo', 'test', '-p', 'manabrew-engine', '--test', 'left_battlefield_this_turn',
                             '--test', 'bottom_departure', '--test', 'prevented_zone_move', '--', '--nocapture'],
                            env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=1700)
    output = result.stdout
    (scratch / 'engine-repair-regressions.log').write_text(output)
    return classify_repair(result.returncode, output)


def classify_repair(code, output):
    if code == 0 and output.count('test result: ok. 2 passed; 0 failed') == 3:
        return True
    if code != 0 and 'test result: FAILED.' in output:
        return False
    raise ValueError('repair regression did not produce a recognized complete result; preserve logs and stop')
