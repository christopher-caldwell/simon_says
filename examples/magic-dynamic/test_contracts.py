#!/usr/bin/env python3
"""Mechanical tests; no models or games."""
import importlib.util
from pathlib import Path
import unittest

here = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('probe', here / 'probe.py')
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


class Classification(unittest.TestCase):
    def test_specific_historical_expression_failure_routes_issues(self):
        self.assertEqual(probe.classify(101, probe.EXPRESSION + '\ntest result: FAILED. 0 passed; 2 failed; 0 ignored'), 'issues')

    def test_passing_regression_routes_clean(self):
        self.assertEqual(probe.classify(0, 'test result: ok. 2 passed; 0 failed; 0 ignored'), 'clean')

    def test_compile_failure_is_not_an_issue(self):
        with self.assertRaises(ValueError):
            probe.classify(101, 'error: failed to compile dependency')

    def test_unknown_test_failure_does_not_guess(self):
        with self.assertRaises(ValueError):
            probe.classify(101, 'test result: FAILED. 0 passed; 2 failed; 0 ignored')

    def test_repair_requires_all_three_frozen_targets(self):
        line = 'test result: ok. 2 passed; 0 failed'
        self.assertTrue(probe.classify_repair(0, line * 3))
        self.assertFalse(probe.classify_repair(101, 'test result: FAILED.'))
        with self.assertRaises(ValueError):
            probe.classify_repair(0, line * 2)
        with self.assertRaises(ValueError):
            probe.classify_repair(101, 'compiler error')

    def test_test_count_or_exit_mismatch_is_not_clean(self):
        for code, output in [(0, 'test result: ok. 0 passed; 0 failed'), (1, 'test result: ok. 2 passed; 0 failed')]:
            with self.subTest(code=code, output=output), self.assertRaises(ValueError):
                probe.classify(code, output)


if __name__ == '__main__':
    unittest.main()
