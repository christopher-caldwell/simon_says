#!/usr/bin/env python3
"""Focused no-model/no-game tests for dynamic game issue and runtime binding."""
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
import game
import native


class Classification(unittest.TestCase):
    def quality(self, diagnostics=None, exclusions=None):
        return {'engineDiagnostics': diagnostics or {}, 'exclusionReasons': exclusions or {}, 'eligibleQualityFlags': {}, 'qualityFlags': {}}

    def test_exact_issue_routes_to_repair(self):
        q = self.quality({game.DIAGNOSTIC: 1}, {'unsupported_engine_behavior': 1})
        self.assertEqual(game.classify('baseline', q), 'issues')
        self.assertEqual(game.classify('corrected', q), 'issues')

    def test_baseline_without_issue_does_not_fabricate_a_fix(self):
        self.assertEqual(game.classify('baseline', self.quality()), 'unresolved')

    def test_corrected_clean_returns_clean(self):
        self.assertEqual(game.classify('corrected', self.quality()), 'clean')

    def test_other_errors_and_exclusions_stop(self):
        self.assertEqual(game.classify('corrected', self.quality({'unknown engine diagnostic': 1})), 'unresolved')
        self.assertEqual(game.classify('corrected', self.quality(exclusions={'incomplete_lifecycle': 1})), 'unresolved')

    def test_mixed_target_and_other_failures_do_not_route(self):
        self.assertEqual(game.classify('baseline', self.quality({game.DIAGNOSTIC: 1}, {'unsupported_engine_behavior': 1, 'incomplete_lifecycle': 1})), 'unresolved')
        self.assertEqual(game.classify('baseline', self.quality({game.DIAGNOSTIC: 1, 'other': 1})), 'unresolved')

    def test_warning_inventory_includes_excluded_attempts(self):
        quality = self.quality(); quality['qualityFlags'] = {'engine_warning': 4}; quality['eligibleQualityFlags'] = {'engine_warning': 1}
        self.assertEqual(game.inventory({'quality': quality})['all_warnings'], {'engine_warning': 4})
        self.assertEqual(game.inventory({'quality': quality})['eligible_warnings'], {'engine_warning': 1})

    def test_pending_or_inconsistent_trial_counts_are_rejected(self):
        denom = {'assignedGames': 10, 'retainedAttempts': 10, 'eligibleGames': 6, 'trialStates': {'accepted': 6, 'excluded': 4}}
        self.assertTrue(game.valid_denominators(denom, 6))
        denom['trialStates']['pending'] = 1
        self.assertFalse(game.valid_denominators(denom, 6))
        del denom['trialStates']['pending']; denom['trialStates']['excluded'] = 3
        self.assertFalse(game.valid_denominators(denom, 6))

    def test_nonempty_wal_must_not_be_ignored(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td); game.require_checkpointed_sqlite(root)
            (root / 'native.sqlite-wal').write_bytes(b'pending')
            with self.assertRaises(ValueError):
                game.require_checkpointed_sqlite(root)

    def test_issue_requires_per_attempt_attribution(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td); attempts = root / 'runs' / 'r' / 'attempts'; attempts.mkdir(parents=True)
            rows = []
            for i in range(10):
                path = attempts / str(i); path.mkdir()
                excluded = i < 4
                (path / 'stderr.log').write_text(game.DIAGNOSTIC if excluded else '')
                rows.append((str(i), i, 'excluded' if excluded else 'accepted', 0 if excluded else 1, str(path), json.dumps(['unsupported_engine_behavior'] if excluded else [])))
            quality = self.quality({game.DIAGNOSTIC: 4}, {'unsupported_engine_behavior': 4})
            self.assertEqual(len(game.attribution(rows, root, 'r', quality, 'issues')), 4)
            (attempts / '0' / 'stderr.log').write_text('')
            with self.assertRaises(ValueError):
                game.attribution(rows, root, 'r', quality, 'issues')
            quality['engineDiagnostics'][game.DIAGNOSTIC] = 3
            with self.assertRaises(ValueError):
                game.attribution(rows, root, 'r', quality, 'issues')

    def test_build_source_must_be_exact_reviewed_repair(self):
        packet = {'baseline_commit': 'good', 'last_transition': {'from_stage': 'repair', 'to_stage': 'build', 'reviewed_commit': 'good'}}
        native.require_repair_transition(packet, 'good')
        for wrong in ['old', '']:
            with self.assertRaises(ValueError):
                native.require_repair_transition(packet, wrong)
        packet['last_transition']['reviewed_commit'] = 'old'
        with self.assertRaises(ValueError):
            native.require_repair_transition(packet, 'good')

    def test_runtime_receipt_and_object_tampering_fail_closed(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td); actions = root / 'actions'; action = actions / 'action-1'; action.mkdir(parents=True)
            packet = action / 'packet.json'; packet.write_text('{}')
            binary = action / 'engine'; binary.write_bytes(b'binary')
            archive = action / 'cards'; archive.write_bytes(b'archive')
            digest = lambda data: hashlib.sha256(data).hexdigest()
            value = {'id': 'id', 'binary': str(binary), 'archive': str(archive),
                     'identity': {'revision': 'good', 'binary_sha256': digest(b'binary'), 'archive_sha256': digest(b'archive')}}
            receipt = action / 'runtime.json'; receipt.write_text(json.dumps(value))
            old = os.environ.get('SIMON_PACKET'); os.environ['SIMON_PACKET'] = str(packet)
            try:
                expected = digest(receipt.read_bytes())
                native.verified_runtime(receipt, expected, 'good')
                with self.assertRaises(ValueError):
                    native.verified_runtime(receipt, expected, 'other')
                binary.write_bytes(b'changed')
                with self.assertRaises(ValueError):
                    native.verified_runtime(receipt, expected, 'good')
                binary.write_bytes(b'binary'); receipt.write_text(json.dumps(value) + ' ')
                with self.assertRaises(ValueError):
                    native.verified_runtime(receipt, expected, 'good')
            finally:
                if old is None:
                    del os.environ['SIMON_PACKET']
                else:
                    os.environ['SIMON_PACKET'] = old


if __name__ == '__main__':
    unittest.main()
