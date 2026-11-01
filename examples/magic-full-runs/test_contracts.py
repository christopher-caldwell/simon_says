#!/usr/bin/env python3
"""Exercise exclusion-preserving handoff without running providers or games."""
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent


class Contracts(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = Path(self.tmp.name)
        sys.path.insert(0, str(root))
        self.addCleanup(sys.path.remove, str(root))
        (root/'runtime_source.py').write_text('def regular_bytes(*a): pass\ndef verify(*a): pass\n')
        spec = importlib.util.spec_from_file_location('full_native_tool', HERE/'native_tool.py')
        self.mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.mod)
        self.actions = root/'run/actions'
        self.store = self.actions/'action-0/scratch/native-store'
        self.native = self.store/'runs/test-run'
        (self.native/'attempts/one').mkdir(parents=True)
        (self.store/'runtime.json').write_text('{}')
        self.packet = self.actions/'action-1/packet.json'
        self.packet.parent.mkdir(parents=True)
        self.packet.write_text('{}')
        self.scratch = self.actions/'action-1/scratch'
        self.scratch.mkdir()
        self.quality = {'exclusionReasons': {'unsupported_engine_behavior': 1}, 'eligibleQualityFlags': {'engine_warning': 2}}
        self.write_native()
        self.collection = {'run': 'test-run', 'store': str(self.store), 'eligible': 3,
                           'excluded': self.quality['exclusionReasons'], 'runtime_identity': 'identity',
                           'issues': [{'kind': 'exclusion', 'details': self.quality['exclusionReasons']},
                                      {'kind': 'eligible_warning', 'details': self.quality['eligibleQualityFlags']}]}
        self.env = patch.dict(os.environ, {'SIMON_PACKET': str(self.packet), 'SIMON_SCRATCH': str(self.scratch)})
        self.env.start(); self.addCleanup(self.env.stop)
        self.mock = patch.object(self.mod, 'run_native', return_value={'run':'test-run', 'verified':4, 'verifiedAttempts':4,
                           'retainedAttempts':4, 'eligibleGames':3, 'rulesCertified':False})
        self.mock.start(); self.addCleanup(self.mock.stop)

    def write_native(self, states=None):
        native = {'run': 'test-run', 'state': 'complete', 'rulesCertified': False, 'runtimeIdentity':'identity',
                  'quality': self.quality, 'denominators': {'assignedGames':4, 'retainedAttempts':4, 'eligibleGames':3,
                  'trialStates': states or {'accepted':3, 'excluded':1}}}
        (self.native/'analysis.json').write_text(json.dumps(native))

    def test_verified_exclusion_and_warnings_are_valid(self):
        self.assertEqual(self.mod.mirror_verify(self.collection)['eligibleGames'], 3)

    def test_repeated_read_only_verification_uses_fresh_scratch(self):
        self.mod.mirror_verify(self.collection)
        self.mod.mirror_verify(self.collection)
        self.assertEqual(len(list(self.scratch.glob("verify-*"))), 2)

    def test_missing_warning_is_rejected(self):
        self.collection['issues'] = self.collection['issues'][:1]
        with self.assertRaisesRegex(ValueError, 'issue inventory'):
            self.mod.mirror_verify(self.collection)

    def test_mismatched_trial_state_is_rejected(self):
        self.write_native({'accepted':3, 'excluded':0, 'pending':1})
        with self.assertRaisesRegex(ValueError, 'denominator'):
            self.mod.mirror_verify(self.collection)

    def test_wrong_native_eligible_count_is_rejected(self):
        self.collection['eligible'] = 2
        with self.assertRaisesRegex(ValueError, 'native verifier'):
            self.mod.mirror_verify(self.collection)


if __name__ == '__main__':
    unittest.main()
