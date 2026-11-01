#!/usr/bin/env python3
"""No-provider, no-game tests for the four-role mechanical handoffs."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

HERE = Path(__file__).resolve().parent


class ReviewContracts(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.repo = Path(self.temp.name)
        for name in ['simon-four', 'simon-game']:
            (self.repo / name).mkdir()
        shutil.copyfile(HERE / 'review.py', self.repo / 'simon-four/review.py')
        shutil.copyfile(HERE / 'verify.py', self.repo / 'simon-four/verify.py')
        subprocess.run(['git', 'init', '-q', str(self.repo)], check=True)
        self.git('config', 'user.name', 'Test')
        self.git('config', 'user.email', 'test@example.invalid')
        (self.repo / 'input.txt').write_text('x')
        self.git('add', 'input.txt')
        self.git('commit', '-qm', 'baseline')
        self.head = self.git('rev-parse', 'HEAD')
        self.packet = {'action_id': 'action-3', 'stage': 'analyze', 'gate': 'reviewer',
                       'candidate_commit': self.head, 'run_context': 'original question'}
        self.game = {'kind': 'issues', 'issues': {'exclusions': {'unsupported_engine_behavior': 4}}, 'eligible': 6}
        self.analysis = {'original_question': 'original question', 'verified_games': 6,
                         'issues': self.game['issues'], 'repair_scope': 'MAGIC-LEFT-BATTLEFIELD'}
        (self.repo / 'simon-game/native.py').write_text("import json\nfrom pathlib import Path\ndef load(p): return json.loads(Path(p).read_text())\ndef frozen_files(): return None, {}\ndef tool_source(files): return None, None, None\n")
        (self.repo / 'simon-game/game.py').write_text("def verify_game(g, cli, env): return {'eligibleGames': 6, 'verifiedAttempts': 10, 'rulesCertified': False}, []\n")
        (self.repo / 'simon-game/review.py').write_text("import json,os\nfrom pathlib import Path\np=json.loads(Path(os.environ['SIMON_PACKET']).read_text())\nprint(json.dumps({'stage':p['stage'],'route':'issues'}))\n")

    def tearDown(self):
        self.temp.cleanup()

    def git(self, *args):
        return subprocess.run(['git', '-C', str(self.repo), *args], check=True,
                              text=True, stdout=subprocess.PIPE).stdout.strip()

    def review(self):
        (self.repo / 'packet.json').write_text(json.dumps(self.packet))
        (self.repo / 'game.json').write_text(json.dumps(self.game))
        (self.repo / 'analysis.json').write_text(json.dumps(self.analysis))
        env = dict(os.environ, SIMON_PACKET=str(self.repo / 'packet.json'))
        proc = subprocess.run(['python3', '-B', 'simon-four/review.py'], cwd=self.repo,
                              env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        return json.loads(proc.stdout)

    def test_exact_evidence_selects_fix(self):
        response = self.review()
        self.assertEqual(response['outcome'], 'pass')
        self.assertEqual(response['route'], 'issues')
        self.assertEqual(response['inspected_commit'], self.head)

    def test_wrong_count_scope_or_inventory_never_routes(self):
        for change in [{'verified_games': 10}, {'repair_scope': 'none'}, {'issues': {}}]:
            with self.subTest(change=change):
                original = self.analysis.copy()
                self.analysis.update(change)
                response = self.review()
                self.assertEqual(response['outcome'], 'changes_required')
                self.assertNotIn('route', response)
                self.analysis = original

    def test_supervisor_delegates_to_native_review(self):
        self.packet['stage'] = 'supervise'
        response = self.review()
        self.assertEqual(response, {'stage': 'supervise', 'route': 'issues'})

    def test_verify_command_requires_expected_argument(self):
        proc = subprocess.run(['python3', '-B', 'simon-four/verify.py'], cwd=self.repo,
                              text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn('use exactly verify', proc.stderr)
        self.assertIn('simon-four/verify.py verify', (HERE / 'analyze.md').read_text())

    def test_investigator_must_bind_exact_configuration(self):
        self.packet['stage'] = 'investigate'
        config = self.repo / 'simon-game/game-config.json'
        config.write_text('{"games":10}')
        design = {'original_question': 'original question', 'planned_games': 10,
                  'config_sha256': hashlib.sha256(config.read_bytes()).hexdigest()}
        (self.repo / 'design.json').write_text(json.dumps(design))
        self.assertEqual(self.review()['outcome'], 'pass')
        design['config_sha256'] = '0' * 64
        (self.repo / 'design.json').write_text(json.dumps(design))
        self.assertEqual(self.review()['outcome'], 'changes_required')


if __name__ == '__main__':
    unittest.main()
