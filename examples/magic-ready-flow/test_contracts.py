#!/usr/bin/env python3
"""No-provider/no-game checks for reviewed post-repair triage routing."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest

HERE = Path(__file__).resolve().parent


class Contract(unittest.TestCase):
    def test_workflow_preserves_reviewed_inputs_and_scopes_warning_route(self):
        w = tomllib.loads((HERE / 'workflow.toml').read_text())
        self.assertEqual(w['stages']['repair']['on_pass'], 'build')
        self.assertEqual(w['stages']['build']['on_pass'], 'simulate')
        self.assertEqual(w['stages']['simulate']['routes'], {'clean': 'complete', 'warnings': 'triage'})
        for stage, required in [('build', ['analysis.json', 'game.json', 'repair.json']),
                                ('simulate', ['analysis.json', 'game.json', 'repair.json', 'native-runtime.json']),
                                ('triage', ['analysis.json', 'game.json', 'repair.json', 'verification.json'])]:
            self.assertTrue(set(required) <= set(w['stages'][stage]['protected_inputs']))
        self.assertEqual(w['stages']['triage']['worker'], 'analyzer')
        self.assertEqual(w['stages']['triage']['on_pass'], 'escalate')
        self.assertEqual(w['stages']['escalate']['worker'], 'escalation')
        self.assertLessEqual(w['max_stage_transitions'], 9)

    def test_invalid_seed_is_rejected_before_project_creation(self):
        with tempfile.TemporaryDirectory() as d:
            target = Path(d) / 'new-project'
            p = subprocess.run(['python3', '-B', str(HERE / 'prepare.py'), '/nonexistent', '/nonexistent',
                                str(target), '--seed', '../bad'], text=True, capture_output=True)
            self.assertNotEqual(p.returncode, 0)
            self.assertIn('seed must be', p.stderr)
            self.assertFalse(target.exists())

    def test_game_reviewer_routes_warning_inventory_without_hiding_it(self):
        source = (HERE.parent / 'magic-dynamic-game/review.py').read_text()
        # Apply the same narrow recorded transformation as the preparer and
        # execute the review protocol against deterministic no-game stubs.
        source = source.replace("if p['stage'] == 'simulate':", "if p['stage'] in ('supervise', 'simulate'):")
        source = source.replace("report = load('game.json')", "report = load('verification.json' if p['stage'] == 'simulate' else 'game.json')")
        source = source.replace("response.update(route=report['kind'], report='Independently verified native game evidence; route: ' + report['kind'])", "route = report['kind']\n        if p['stage'] == 'simulate' and (route == 'issues' or any(report['issues']['all_warnings'].values())):\n            route = 'warnings'\n        response.update(route=route, report='Independently verified native game evidence; route: ' + route)")
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            subprocess.run(['git', 'init', '-q', str(root)], check=True)
            subprocess.run(['git', '-C', str(root), 'config', 'user.name', 'Test'], check=True)
            subprocess.run(['git', '-C', str(root), 'config', 'user.email', 'test@example.invalid'], check=True)
            (root / 'x').write_text('x')
            subprocess.run(['git', '-C', str(root), 'add', 'x'], check=True)
            subprocess.run(['git', '-C', str(root), 'commit', '-qm', 'baseline'], check=True)
            head = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
            (root / 'review.py').write_text(source)
            (root / 'game.py').write_text('def classify(*a): pass\ndef verify_game(*a): return None\n')
            (root / 'native.py').write_text('import json\nfrom pathlib import Path\ndef load(p): return json.loads(Path(p).read_text())\ndef frozen_files(): return None, {"store/runtime.json": b"{\\"id\\":\\"runtime\\"}"}\ndef tool_source(f): return None,None,None\ndef regular(p): return b"config"\ndef sha(p): return "hash"\ndef verified_runtime(*a): return {}\ndef require_repair_transition(*a): pass\n')
            (root / 'probe.py').write_text('def run_repair(): return True\n')
            (root / 'packet.json').write_text(json.dumps({'stage':'simulate', 'gate':'reviewer', 'candidate_commit':head, 'action_id':'a'}))
            env = dict(os.environ, SIMON_PACKET=str(root / 'packet.json'))
            for kind, warnings, expected in [('clean', {}, 'clean'), ('clean', {'engine_warning': 2}, 'warnings'), ('issues', {}, 'warnings')]:
                report = {'kind':kind, 'phase':'baseline', 'runtime_id':'runtime', 'config_sha256':'hash', 'issues':{'all_warnings':warnings}}
                (root / 'verification.json').write_text(json.dumps(report))
                p = subprocess.run(['python3', '-B', 'review.py'], cwd=root, env=env, text=True, capture_output=True)
                self.assertEqual(p.returncode, 0, p.stderr)
                self.assertEqual(json.loads(p.stdout)['route'], expected)
            report['kind'] = 'unresolved'
            (root / 'verification.json').write_text(json.dumps(report))
            p = subprocess.run(['python3', '-B', 'review.py'], cwd=root, env=env, text=True, capture_output=True)
            self.assertEqual(json.loads(p.stdout)['outcome'], 'blocked')

    def test_triage_cannot_lose_issue_inventory(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            (root / 'simon-game').mkdir()
            subprocess.run(['git', 'init', '-q', str(root)], check=True)
            subprocess.run(['git', '-C', str(root), 'config', 'user.name', 'Test'], check=True)
            subprocess.run(['git', '-C', str(root), 'config', 'user.email', 'test@example.invalid'], check=True)
            (root / 'x').write_text('x')
            subprocess.run(['git', '-C', str(root), 'add', 'x'], check=True)
            subprocess.run(['git', '-C', str(root), 'commit', '-qm', 'baseline'], check=True)
            head = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
            (root / 'review.py').write_text((HERE / 'review.py').read_text())
            (root / 'simon-game/game.py').write_text("def verify_game(*args): return {'eligibleGames': 8}, []\n")
            (root / 'simon-game/native.py').write_text("import json\nfrom pathlib import Path\ndef load(p): return json.loads(Path(p).read_text())\ndef frozen_files(): return None,{}\ndef tool_source(f): return None,None,None\n")
            (root / 'packet.json').write_text(json.dumps({'stage': 'triage', 'gate': 'reviewer',
                'action_id': 'a', 'candidate_commit': head, 'run_context': 'question'}))
            issues = {'all_warnings': {'engine_warning': 2}}
            (root / 'verification.json').write_text(json.dumps({'issues': issues}))
            env = dict(os.environ, SIMON_PACKET=str(root / 'packet.json'))
            for count, saved, expected in [(8, issues, 'pass'), (7, issues, 'changes_required'), (8, {}, 'changes_required')]:
                (root / 'triage.json').write_text(json.dumps({'original_question': 'question', 'eligible_games': count, 'issues': saved}))
                p = subprocess.run(['python3', '-B', 'review.py'], cwd=root, env=env, text=True, capture_output=True)
                self.assertEqual(p.returncode, 0, p.stderr)
                self.assertEqual(json.loads(p.stdout)['outcome'], expected)
        self.assertIn('simon-four/verify.py verify-post', (HERE / 'triage.md').read_text())


if __name__ == '__main__':
    unittest.main()
