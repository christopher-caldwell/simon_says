#!/usr/bin/env python3
"""No-game checks for evidence binding and analysis-only invocation."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

PROJECT = Path(os.environ['SIMON_ANALYSIS_PROJECT']).resolve(strict=True)
sys.path.insert(0, str(PROJECT))
spec = importlib.util.spec_from_file_location('diagnostic_analysis_native', PROJECT / 'native_tool.py')
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)


class Contracts(unittest.TestCase):
    def test_collect_command_is_not_supported(self):
        result = subprocess.run([sys.executable, '-B', str(PROJECT / 'native_tool.py'), 'collect'], text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('supports only verify', result.stderr)

    def test_manifest_detects_mutation_and_new_file(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            (root / 'one').write_text('original')
            manifest = {'store': str(root), 'run': 'run', 'files': {'one': hashlib.sha256(b'original').hexdigest()}}
            with patch.object(native, 'load', return_value=manifest):
                self.assertEqual(native.evidence(), (root.resolve(), 'run'))
                (root / 'one').write_text('changed')
                with self.assertRaisesRegex(ValueError, 'differs'):
                    native.evidence()
                (root / 'one').write_text('original')
                (root / 'new').write_text('extra')
                with self.assertRaisesRegex(ValueError, 'differs'):
                    native.evidence()

    def test_manifest_rejects_symlink(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            (root / 'one').symlink_to('/dev/null')
            with patch.object(native, 'load', return_value={'store': str(root), 'run': 'run', 'files': {}}):
                with self.assertRaisesRegex(ValueError, 'symlink'):
                    native.evidence()

    def test_complete_issue_inventory_is_bound(self):
        collection = json.loads((PROJECT / 'collection.json').read_text())
        manifest = json.loads((PROJECT / 'evidence-manifest.json').read_text())
        evidence = json.loads((Path(manifest['store']) / 'runs' / manifest['run'] / 'analysis.json').read_text())
        self.assertEqual((collection['assigned'], collection['attempts'], collection['eligible']), (10, 10, 6))
        self.assertEqual(collection['issues'], [
            {'kind': 'exclusion', 'details': evidence['quality']['exclusionReasons']},
            {'kind': 'eligible_warning', 'details': evidence['quality']['eligibleQualityFlags']},
        ])


if __name__ == '__main__':
    unittest.main()
