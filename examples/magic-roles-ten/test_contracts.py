#!/usr/bin/env python3
"""Check the adapted review and collection bounds without models or games."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

PROJECT = Path(os.environ['SIMON_TEN_PROJECT']).resolve(strict=True)


def module(name):
    spec = importlib.util.spec_from_file_location(name, PROJECT / (name + '.py'))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


class Contracts(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.previous = Path.cwd()
        os.chdir(self.temp.name)
        self.addCleanup(os.chdir, self.previous)
        self.addCleanup(self.temp.cleanup)
        self.config = {'games': 10, 'purpose': 'diagnostic'}
        self.write_config()
        self.design = {'original_question': 'question', 'config_sha256': self.digest(), 'planned_games': 10}
        self.write_design()

    def write_config(self):
        Path('diagnostic.json').write_text(json.dumps(self.config))

    def write_design(self):
        Path('design.json').write_text(json.dumps(self.design))

    def digest(self):
        return hashlib.sha256(Path('diagnostic.json').read_bytes()).hexdigest()

    def test_review_checks_exact_config_and_ten_games(self):
        review = module('review')
        self.assertIn('ten-game', review.review_stage('investigate', {'run_context': 'question'}))
        self.design['planned_games'] = 1
        self.write_design()
        with self.assertRaises(review.ChangesRequired):
            review.review_stage('investigate', {'run_context': 'question'})
        self.design['planned_games'] = 10
        self.design['config_sha256'] = '0' * 64
        self.write_design()
        with self.assertRaises(review.ChangesRequired):
            review.review_stage('investigate', {'run_context': 'question'})

    def test_collection_refuses_changed_game_count_before_runtime(self):
        import sys
        sys.path.insert(0, str(PROJECT))
        self.addCleanup(sys.path.remove, str(PROJECT))
        native = module('native_tool')
        self.config['games'] = 11
        self.write_config()
        self.design['config_sha256'] = self.digest()
        self.write_design()
        with patch.object(native, 'packet', return_value={'stage': 'supervise', 'gate': 'worker', 'run_context': 'question'}), patch.object(native, 'run_native') as invoke:
            with self.assertRaisesRegex(ValueError, 'ten diagnostic games only'):
                native.collect()
            invoke.assert_not_called()


if __name__ == '__main__':
    unittest.main()
