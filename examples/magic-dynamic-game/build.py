#!/usr/bin/env python3
"""Build the exact accepted repair commit with the maintained native setup, in action scratch."""
import json
import os
from pathlib import Path
import subprocess
from native import call, frozen_files, load, regular, sha, tool_source, verified_runtime, require_repair_transition

p = load(os.environ['SIMON_PACKET'])
if p['stage'] != 'build' or p['gate'] != 'worker':
    raise ValueError('only build Worker may construct runtime')
require_repair_transition(p, p['baseline_commit'])
subprocess.run(['git', 'cat-file', '-e', p['baseline_commit'] + '^{commit}'], check=True)
root, files = frozen_files()
source, cli, env = tool_source(files)
spec_path = source / 'repo/sim/manabrew/engine-source.json'
spec = load(spec_path)
if spec.get('revision') != '44f09e473213606382168f0b08442057823b9128':
    raise ValueError('unexpected original native source pin')
spec['revision'] = p['baseline_commit']
spec_path.write_text(json.dumps(spec, indent=2) + '\n')
baseline = json.loads(files['store/runtime.json'])
archive = Path(baseline['archive']).resolve(strict=True)
store = Path(os.environ['SIMON_SCRATCH']) / 'candidate-runtime'
store.mkdir()
receipt = call(cli, env, store, 'setup', '--archive', archive, '--source-cache', Path.cwd(), timeout=7000)
receipt_path = store / 'runtime.json'
receipt_hash = sha(regular(receipt_path))
checked = verified_runtime(receipt_path, receipt_hash, p['baseline_commit'])
if receipt['id'] != checked['id'] or checked['identity']['archive_sha256'] != baseline['identity']['archive_sha256']:
    raise ValueError('candidate receipt or archive mismatch')
# Recheck authoritative baseline source after the lengthy build.
frozen_files()
record = {'source_commit': p['baseline_commit'], 'receipt_path': str(receipt_path), 'receipt_sha256': receipt_hash,
          'runtime_id': checked['id'], 'binary_sha256': checked['identity']['binary_sha256'],
          'archive_sha256': checked['identity']['archive_sha256']}
Path('native-runtime.json').write_text(json.dumps(record, indent=2) + '\n')
subprocess.run(['git', 'add', '--', 'native-runtime.json'], check=True)
subprocess.run(['git', '-c', 'commit.gpgsign=false', 'commit', '--allow-empty', '-m', 'Record verified isolated runtime'], check=True, stdout=subprocess.PIPE)
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
print(json.dumps({'action_id': p['action_id'], 'outcome': 'complete', 'report': 'Built isolated native runtime from exact accepted source commit.',
                  'candidate_commit': head, 'inspected_commit': None}))
