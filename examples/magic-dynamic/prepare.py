#!/usr/bin/env python3
"""Prepare a new isolated historic-engine project and bind real retained game evidence."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib

EXPRESSION = b'Unrecognized Count expression, returning 0 for: Count$LeftBattlefieldThisTurn Creature.YouCtrl'


def git(*args, cwd=None):
    return subprocess.run(['git', *map(str, args)], cwd=cwd, text=True, capture_output=True, check=True).stdout.strip()


def make_manifest(run):
    run = run.resolve(strict=True)
    if run.is_symlink() or not run.is_dir():
        raise ValueError('unsafe evidence run directory')
    analysis = run / 'analysis.json'
    data = json.loads(analysis.read_bytes())
    if data['quality']['exclusionReasons'].get('unsupported_engine_behavior') != 4:
        raise ValueError('expected four historical unsupported-engine exclusions')
    matches = []
    attempts = run / 'attempts'
    for path in sorted(attempts.glob('*/stderr.log')):
        if path.is_symlink() or path.parent.is_symlink():
            raise ValueError('unsafe evidence path')
        b = path.read_bytes()
        if EXPRESSION in b:
            matches.append({'attempt': path.parent.name, 'stderr_sha256': hashlib.sha256(b).hexdigest()})
    if len(matches) != 4:
        raise ValueError('expected four retained attempts with the exact recorded engine diagnostic')
    return {'source_run': data['run'], 'analysis_sha256': hashlib.sha256(analysis.read_bytes()).hexdigest(),
            'source_commit': '44f09e473213606382168f0b08442057823b9128',
            'diagnostic': EXPRESSION.decode(), 'attempts': matches,
            'limitations': 'Historical retained evidence only; correlation does not establish game outcome impact.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('engine_repo', type=Path)
    parser.add_argument('collection_repo', type=Path)
    parser.add_argument('retained_run', type=Path, help='existing native run directory, read-only')
    parser.add_argument('new_project', type=Path)
    args = parser.parse_args()
    manifest = make_manifest(args.retained_run)
    here = Path(__file__).resolve().parent
    subprocess.run([sys.executable, '-B', str(here.parent / 'magic-real-fix' / 'prepare.py'),
                    str(args.engine_repo), str(args.collection_repo), str(args.new_project)], check=True)
    target = args.new_project.resolve(strict=True)
    dest = target / 'simon-dynamic'
    dest.mkdir()
    for name in ['probe.py', 'observe.py', 'review.py', 'stop.py', 'observe.md', 'repair.md', 'finding.schema.json', 'repair.schema.json']:
        shutil.copyfile(here / name, dest / name)
    tests = []
    for name in ('left_battlefield_this_turn.rs', 'bottom_departure.rs', 'prevented_zone_move.rs'):
        test_path = target / 'manabrew-rs/crates/manabrew-engine/tests' / name
        if test_path.exists() or test_path.is_symlink():
            raise ValueError('unexpected pre-existing regression target')
        shutil.copyfile(here / name, test_path)
        tests.append(str(test_path.relative_to(target)))
    (dest / 'evidence.json').write_text(json.dumps(manifest, indent=2) + '\n')
    role = tomllib.loads((target / 'simon-workflow.toml').read_text())['roles']['fixer']
    text = (here / 'workflow.toml').read_text().replace('model = "gpt-6.1-terra"', 'model = ' + json.dumps(role['model'])).replace('effort = "high"', 'effort = ' + json.dumps(role['effort']))
    (target / 'workflow.toml').write_text(text)
    if make_manifest(args.retained_run) != manifest:
        raise ValueError('retained source evidence changed during preparation')
    git('add', '--', 'simon-dynamic', 'workflow.toml', *tests, cwd=target)
    git('commit', '-m', 'Prepare isolated evidence-driven engine repair workflow', cwd=target)
    print(json.dumps({'project': str(target), 'baseline': git('rev-parse', 'HEAD', cwd=target),
                      'source_run': manifest['source_run'], 'source_attempts': len(manifest['attempts'])}))


if __name__ == '__main__':
    main()
