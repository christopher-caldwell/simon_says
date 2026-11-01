#!/usr/bin/env python3
"""Freeze a real native baseline and prepare a disposable historical engine project."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib
from native import capture, manifest


def git(*args, cwd=None):
    return subprocess.run(['git', *map(str, args)], cwd=cwd, text=True, capture_output=True, check=True).stdout.strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('engine_repo', type=Path)
    parser.add_argument('collection_repo', type=Path)
    parser.add_argument('retained_run', type=Path)
    parser.add_argument('baseline_runtime', type=Path)
    parser.add_argument('prepared_ten_game_project', type=Path)
    parser.add_argument('new_project', type=Path)
    args = parser.parse_args()
    here = Path(__file__).resolve().parent
    runtime = args.baseline_runtime.resolve(strict=True)
    reference = args.prepared_ten_game_project.resolve(strict=True)
    requested = args.new_project.absolute()
    target = requested.parent.resolve(strict=True) / requested.name
    if target.exists() or target.is_symlink() or target.is_relative_to(runtime) or target.is_relative_to(reference):
        raise ValueError('destination must be new and outside reference runtime/project')
    if git('status', '--porcelain=v1', cwd=reference):
        raise ValueError('prepared ten-game reference project is not clean')
    files = capture(runtime)
    receipt = json.loads(files['store/runtime.json'])
    if receipt['identity']['revision'] != '44f09e473213606382168f0b08442057823b9128':
        raise ValueError('baseline runtime is not the recorded historical engine')
    config = json.loads((reference / 'diagnostic.json').read_text())
    if (config.get('games') != 10 or config.get('workers') != 1 or config.get('purpose') != 'diagnostic'
            or config.get('seedNamespace') != 'simon-says-ten-game-diagnostic-20261101'):
        raise ValueError('ten-game diagnostic config does not match recorded reproducer')
    historic = json.loads((args.retained_run / 'analysis.json').read_bytes())
    historic_manifest = json.loads((args.retained_run / 'manifest.json').read_bytes())
    if receipt['id'] != historic['runtimeIdentity'] or receipt['id'] != historic_manifest['runtime']['id']:
        raise ValueError('baseline runtime differs from retained historical run')
    if any(historic_manifest['config'].get(key) != value for key, value in config.items()):
        raise ValueError('baseline game configuration differs from retained historical run')
    historic_inputs = {item['id']: item for item in historic['inputs']}
    for deck, checkpoint in [('angels', 2), ('miirym', 2), ('azula', 1), ('aang', 1)]:
        relative = Path('decks') / deck / f'checkpoint_{checkpoint}'
        item = historic_inputs[f'{deck}-cp{checkpoint}']
        for filename, key in [('deck_list.csv', 'csv_sha256'), ('GUIDE.md', 'guide_sha256')]:
            import hashlib
            if hashlib.sha256((reference / relative / filename).read_bytes()).hexdigest() != item[key]:
                raise ValueError('baseline deck or guide differs from retained historical run')
    before = manifest(files)
    subprocess.run([sys.executable, '-B', str(here.parent / 'magic-dynamic' / 'prepare.py'),
                    str(args.engine_repo), str(args.collection_repo), str(args.retained_run), str(target)], check=True)
    dest = target / 'simon-game'
    dest.mkdir()
    for name in ['native.py', 'game.py', 'build.py', 'review.py', 'probe.py', 'stop.py', 'ps_compat.py',
                 'simulate.md', 'repair.md', 'build.md', 'game.schema.json', 'repair.schema.json', 'runtime.schema.json']:
        shutil.copyfile(here / name, dest / name)
    (dest / 'assets.json').write_text(json.dumps({'root': str(runtime), 'files': before}, indent=2) + '\n')
    # Reuse the previously observed seed for a newly authorized comparison,
    # with independent stores before/after; never replay an uncertain action.
    shutil.copyfile(reference / 'diagnostic.json', dest / 'game-config.json')
    for deck, checkpoint in [('angels', 2), ('miirym', 2), ('azula', 1), ('aang', 1)]:
        relative = Path('decks') / deck / f'checkpoint_{checkpoint}'
        (target / relative).mkdir(parents=True)
        for name in ['deck_list.csv', 'GUIDE.md']:
            shutil.copyfile(reference / relative / name, target / relative / name)
    role = tomllib.loads((target / 'simon-workflow.toml').read_text())['roles']['fixer']
    workflow = (here / 'workflow.toml').read_text().replace('model = "gpt-6.1-terra"', 'model = ' + json.dumps(role['model'])).replace('effort = "high"', 'effort = ' + json.dumps(role['effort']))
    (target / 'workflow.toml').write_text(workflow)
    if manifest(capture(runtime)) != before:
        raise ValueError('baseline runtime changed during preparation')
    git('add', '--', 'simon-game', 'workflow.toml', 'decks', cwd=target)
    git('commit', '-m', 'Prepare native game driven repair workflow', cwd=target)
    print(json.dumps({'project': str(target), 'baseline': git('rev-parse', 'HEAD', cwd=target),
                      'runtime': receipt['id'], 'native_files': len(before)}))


if __name__ == '__main__':
    main()
