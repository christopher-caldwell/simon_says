#!/usr/bin/env python3
"""No-provider/no-game tests for direct controller-supervised collection."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('prepare_dispatch_test', HERE / 'prepare.py')
prepare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prepare)


def git(root, *args):
    return subprocess.run(['git', '-C', str(root), *args], check=True, text=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout.strip()


class DirectCollection(unittest.TestCase):
    def test_all_downstream_stages_must_protect_the_reviewed_plan(self):
        stages = ['collect', 'analyze', 'repair', 'build', 'simulate', 'triage', 'escalate',
                  'repair_origin', 'build_origin', 'simulate_origin', 'triage_final', 'escalate_final']
        source = ''.join(f'[stages.{name}]\ninputs = ["x"]\nprotected_inputs = ["x"]\n' for name in stages)
        out = prepare.add_protected_plan(source)
        self.assertEqual(out.count('"collection-plan.json"'), 2 * len(stages))
        with self.assertRaises(ValueError):
            prepare.add_protected_plan(source.replace('[stages.collect]\n', '[stages.other]\n'))

    def test_supervisor_review_checks_question_config_and_single_launch(self):
        source = prepare.adapt_review((HERE.parent / 'magic-ready-origin/review.py').read_text())
        for patch, outcome in [({}, 'pass'), ({'assigned_games': 11}, 'changes_required'),
                               ({'config_sha256': '0' * 64}, 'changes_required'),
                               ({'original_question': 'other'}, 'changes_required'),
                               ({'launch_policy': 'retry'}, 'changes_required')]:
            with self.subTest(patch=patch), tempfile.TemporaryDirectory() as d:
                root = Path(d); (root / 'simon-game').mkdir()
                git(root, 'init', '-q'); git(root, 'config', 'user.name', 'Test'); git(root, 'config', 'user.email', 'test@example.invalid')
                config = b'{\"games\":10}\n'; digest = hashlib.sha256(config).hexdigest()
                (root / 'simon-game/game-config.json').write_bytes(config)
                (root / 'design.json').write_text(json.dumps({'original_question':'question','config_sha256':digest}))
                git(root, 'add', '.'); git(root, 'commit', '-qm', 'baseline'); baseline = git(root, 'rev-parse', 'HEAD')
                plan = {'original_question':'question','config_sha256':digest,'assigned_games':10,'launch_policy':'once_no_retries','limitations':'diagnostic'}
                plan.update(patch); (root / 'collection-plan.json').write_text(json.dumps(plan))
                git(root, 'add', 'collection-plan.json'); git(root, 'commit', '-qm', 'plan'); head = git(root, 'rev-parse', 'HEAD')
                (root / 'review.py').write_text(source)
                (root / 'packet.json').write_text(json.dumps({'stage':'supervise','gate':'reviewer','action_id':'a',
                    'candidate_commit':head,'baseline_commit':baseline,'run_context':'question'}))
                env = dict(os.environ, SIMON_PACKET=str(root / 'packet.json'))
                p = subprocess.run(['python3','-B','review.py'], cwd=root, env=env, text=True, capture_output=True)
                self.assertEqual(p.returncode, 0, p.stderr)
                self.assertEqual(json.loads(p.stdout)['outcome'], outcome)

    def test_inconsistent_plan_fails_before_native_runtime_is_touched(self):
        source = (HERE.parent / 'magic-dynamic-game/game.py').read_text()
        source = source.replace("packet['stage'] != 'simulate'", "packet['stage'] not in ('supervise', 'simulate', 'simulate_origin')")
        source = prepare.adapt_game(source)
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); (root / 'simon-game').mkdir()
            (root / 'game.py').write_text(source)
            (root / 'native.py').write_text('''import hashlib,json
from pathlib import Path
def load(p): return json.loads(Path(p).read_text())
def regular(p): return Path(p).read_bytes()
def sha(b): return hashlib.sha256(b).hexdigest()
def frozen_files(): raise RuntimeError("NATIVE_RUNTIME_REACHED")
def action_root(): pass
def call(*a): pass
def tool_source(*a): pass
def verified_runtime(*a): pass
''')
            config = b'{\"games\":10}\n'; (root / 'simon-game/game-config.json').write_bytes(config)
            digest = hashlib.sha256(config).hexdigest()
            (root / 'design.json').write_text(json.dumps({'original_question':'question','config_sha256':digest}))
            (root / 'collection-plan.json').write_text(json.dumps({'original_question':'question','config_sha256':'0'*64,'assigned_games':10,'launch_policy':'once_no_retries'}))
            (root / 'packet.json').write_text(json.dumps({'stage':'collect','gate':'worker','run_context':'question'}))
            env = dict(os.environ, SIMON_PACKET=str(root / 'packet.json'))
            p = subprocess.run(['python3','-B','game.py'], cwd=root, env=env, text=True, capture_output=True)
            self.assertNotEqual(p.returncode, 0)
            self.assertIn('reviewed collection plan is inconsistent', p.stderr)
            self.assertNotIn('NATIVE_RUNTIME_REACHED', p.stderr)
            self.assertFalse((root / 'native-store').exists())


if __name__ == '__main__':
    unittest.main()
