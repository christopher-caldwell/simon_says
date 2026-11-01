#!/usr/bin/env python3
"""No-provider, no-game controls for scoped Origin evidence and routing."""
import importlib.util
import json
import os
import subprocess
import tempfile
import sys
from pathlib import Path
import types
import unittest

HERE = Path(__file__).resolve().parent
native = types.ModuleType('native')
for name in ('action_root', 'frozen_files', 'load', 'regular', 'sha', 'tool_source'):
    setattr(native, name, lambda *args: None)
previous = sys.modules.get('native')
sys.modules['native'] = native
spec = importlib.util.spec_from_file_location('origin_probe_test', HERE / 'origin_probe.py')
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)
if previous is None:
    del sys.modules['native']
else:
    sys.modules['native'] = previous


class OriginContracts(unittest.TestCase):
    def test_frozen_control_accepts_only_expected_failing_test(self):
        expected = ('archived_origin_not_offered_without_any_legal_mode ... FAILED\n'
                    'archived_origin_offered_when_artifact_target_is_available ... ok\n'
                    'archived_boros_charm_offered_when_non_target_mode_is_available ... ok\n'
                    'test result: FAILED. 2 passed; 1 failed;')
        self.assertEqual(probe.classify_controls(101, expected), 'reproduced')
        self.assertEqual(probe.classify_controls(0, 'test result: ok. 3 passed; 0 failed;'), 'passed')
        for status, output in [(0, expected), (101, expected.replace('2 passed; 1 failed', '1 passed; 2 failed')),
                               (101, expected.replace('artifact_target_is_available ... ok', 'artifact_target_is_available ... FAILED')),
                               (101, 'compiler crashed')]:
            with self.subTest(status=status, output=output):
                with self.assertRaises(ValueError):
                    probe.classify_controls(status, output)

    def test_no_route_without_both_live_warning_and_reproduced_control(self):
        self.assertEqual(probe.recommendation([{'attempt': 'a'}], 'reproduced'), 'investigate')
        self.assertEqual(probe.recommendation([], 'reproduced'), 'unresolved')
        self.assertEqual(probe.recommendation([{'attempt': 'a'}], 'passed'), 'unresolved')

    def test_persisting_target_issue_blocks_before_origin_route(self):
        source = (HERE.parent / 'magic-dynamic-game/review.py').read_text()
        source = source.replace("if p['stage'] == 'simulate':", "if p['stage'] in ('supervise', 'simulate'):")
        source = source.replace("report = load('game.json')", "report = load('verification.json' if p['stage'] == 'simulate' else 'game.json')")
        old = "response.update(route=report['kind'], report='Independently verified native game evidence; route: ' + report['kind'])"
        source = source.replace(old, "route = report['kind']\n        if p['stage'] == 'simulate' and (route == 'issues' or any(report['issues']['all_warnings'].values())):\n            route = 'warnings'\n        response.update(route=route, report='Independently verified native game evidence; route: ' + route)")
        sp = importlib.util.spec_from_file_location('prepare_origin_test', HERE / 'prepare.py')
        prepare = importlib.util.module_from_spec(sp)
        sp.loader.exec_module(prepare)
        source = prepare.transform_game_review(source)
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
            (root / 'native.py').write_text('''import json
from pathlib import Path
def load(p): return json.loads(Path(p).read_text())
def frozen_files(): return None, {"store/runtime.json": b'{"id":"runtime"}'}
def tool_source(f): return None,None,None
def regular(p): return b"config"
def sha(p): return "hash"
def verified_runtime(*a): return {}
def require_repair_transition(*a): pass
''')
            (root / 'probe.py').write_text('def run_repair(): return True\n')
            env = dict(os.environ, SIMON_PACKET=str(root / 'packet.json'))
            for stage in ('simulate', 'simulate_origin'):
                (root / 'packet.json').write_text(json.dumps({'stage': stage, 'gate':'reviewer', 'candidate_commit':head, 'action_id':'a'}))
                name = 'verification-origin.json' if stage == 'simulate_origin' else 'verification.json'
                for kind, warnings, expected in [('issues', {}, 'blocked'), ('unresolved', {}, 'blocked'),
                                                 ('clean', {'engine_warning': 2}, 'warnings'), ('clean', {}, 'clean')]:
                    report = {'kind':kind, 'phase':'baseline', 'runtime_id':'runtime', 'config_sha256':'hash', 'issues':{'all_warnings':warnings}}
                    (root / name).write_text(json.dumps(report))
                    result = subprocess.run(['python3', '-B', 'review.py'], cwd=root, env=env, text=True, capture_output=True)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    parsed = json.loads(result.stdout)
                    self.assertEqual(parsed['outcome'] if expected == 'blocked' else parsed['route'], expected)

    def test_triage_dispatch_requires_matching_probe_and_analyzer_decision(self):
        for recommendation, action, expected in [('investigate', 'investigate', 'origin'),
                                                  ('unresolved', 'investigate', 'changes_required'),
                                                  ('investigate', 'unresolved', 'unresolved')]:
            with self.subTest(recommendation=recommendation, action=action), tempfile.TemporaryDirectory() as d:
                root = Path(d)
                (root / 'simon-game').mkdir(); (root / 'simon-four').mkdir()
                subprocess.run(['git', 'init', '-q', str(root)], check=True)
                subprocess.run(['git', '-C', str(root), 'config', 'user.name', 'Test'], check=True)
                subprocess.run(['git', '-C', str(root), 'config', 'user.email', 'test@example.invalid'], check=True)
                issues = {'all_warnings': {'card_play_failed': 1}}
                (root / 'verification.json').write_text(json.dumps({'issues': issues}))
                subprocess.run(['git', '-C', str(root), 'add', 'verification.json'], check=True)
                subprocess.run(['git', '-C', str(root), 'commit', '-qm', 'baseline'], check=True)
                baseline = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
                evidence = {'origin_warnings': [{'attempt': 'a', 'seq': 1}], 'control': 'reproduced',
                            'recommendation': recommendation, 'source_tree': 'tree'}
                (root / 'triage.json').write_text(json.dumps({'original_question': 'question', 'eligible_games': 8,
                    'issues': issues, 'origin_action': action, 'origin_evidence': evidence}))
                subprocess.run(['git', '-C', str(root), 'add', 'triage.json'], check=True)
                subprocess.run(['git', '-C', str(root), 'commit', '-qm', 'candidate'], check=True)
                head = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
                (root / 'review.py').write_text((HERE / 'review.py').read_text())
                (root / 'simon-game/game.py').write_text("def verify_game(*args): return {'eligibleGames': 8}, []\n")
                (root / 'simon-game/native.py').write_text("import json\nfrom pathlib import Path\ndef load(p): return json.loads(Path(p).read_text())\ndef frozen_files(): return None,{}\ndef tool_source(f): return None,None,None\n")
                (root / 'simon-four/origin_probe.py').write_text('import json\nprint(json.dumps(' + repr(evidence) + '))\n')
                (root / 'packet.json').write_text(json.dumps({'stage': 'triage', 'gate': 'reviewer', 'action_id': 'a',
                    'candidate_commit': head, 'baseline_commit': baseline, 'run_context': 'question'}))
                env = dict(os.environ, SIMON_PACKET=str(root / 'packet.json'))
                result = subprocess.run(['python3', '-B', 'review.py'], cwd=root, env=env, text=True, capture_output=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                parsed = json.loads(result.stdout)
                self.assertEqual(parsed['outcome'] if expected == 'changes_required' else parsed['route'], expected)

    def test_triage_cannot_change_source_under_cover_of_an_output(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            subprocess.run(['git', 'init', '-q', str(root)], check=True)
            subprocess.run(['git', '-C', str(root), 'config', 'user.name', 'Test'], check=True)
            subprocess.run(['git', '-C', str(root), 'config', 'user.email', 'test@example.invalid'], check=True)
            (root / 'engine.rs').write_text('before')
            subprocess.run(['git', '-C', str(root), 'add', '.'], check=True)
            subprocess.run(['git', '-C', str(root), 'commit', '-qm', 'baseline'], check=True)
            baseline = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
            (root / 'engine.rs').write_text('unauthorized source change')
            (root / 'triage.json').write_text('{}')
            subprocess.run(['git', '-C', str(root), 'add', '.'], check=True)
            subprocess.run(['git', '-C', str(root), 'commit', '-qm', 'bad candidate'], check=True)
            head = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
            (root / 'review.py').write_text((HERE / 'review.py').read_text())
            (root / 'packet.json').write_text(json.dumps({'stage': 'triage', 'gate': 'reviewer',
                'candidate_commit': head, 'baseline_commit': baseline, 'action_id': 'a'}))
            env = dict(os.environ, SIMON_PACKET=str(root / 'packet.json'))
            result = subprocess.run(['python3', '-B', 'review.py'], cwd=root, env=env, text=True, capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('non-repair role modified paths', result.stderr)

    def test_distinct_runtime_and_evidence_paths(self):
        text = (HERE / 'extra-stages.toml').read_text()
        self.assertIn('outputs = ["native-runtime-origin.json"]', text)
        self.assertIn('outputs = ["verification-origin.json"]', text)
        self.assertIn('"verification.json", "triage.json", "warning-repair.json"', text)
        self.assertIn('on_pass = "build_origin"', text)
        self.assertIn('on_pass = "simulate_origin"', text)


if __name__ == '__main__':
    unittest.main()
