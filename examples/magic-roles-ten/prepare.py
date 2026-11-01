#!/usr/bin/env python3
"""Apply the recorded ten-game diagnostic adaptation to a prepared role project."""
import argparse
import json
from pathlib import Path
import subprocess


def git(*args, cwd=None):
    return subprocess.run(['git', *map(str, args)], cwd=cwd, text=True, check=True, capture_output=True).stdout.strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('prepared_one_game_project', type=Path)
    parser.add_argument('new_project', type=Path)
    args = parser.parse_args()
    source = args.prepared_one_game_project.resolve(strict=True)
    target = args.new_project.resolve(strict=False)
    if target.exists() or target.is_relative_to(source):
        raise ValueError('new project must not exist or be inside the prepared project')
    if git('status', '--porcelain=v1', cwd=source):
        raise ValueError('prepared project must have a clean working tree')
    patch = Path(__file__).resolve().with_name('ten-game.patch').read_bytes()
    subprocess.run(['git', 'clone', '--no-hardlinks', '--', str(source), str(target)], check=True)
    subprocess.run(['git', 'apply', '--check', '-'], cwd=target, input=patch, check=True)
    subprocess.run(['git', 'apply', '-'], cwd=target, input=patch, check=True)
    config = json.loads((target / 'diagnostic.json').read_text())
    if config['games'] != 10 or config['workers'] != 1 or config['purpose'] != 'diagnostic':
        raise ValueError('adapted diagnostic has unexpected bounds')
    for filename, fields in [('design.schema.json', ['planned_games']), ('collection.schema.json', ['assigned', 'attempts', 'eligible']), ('analysis.schema.json', ['verified_games'])]:
        schema = json.loads((target / filename).read_text())
        if any(schema['properties'][name].get('const') != 10 for name in fields):
            raise ValueError(f'incorrect ten-game contract: {filename}')
    git('add', '--all', cwd=target)
    git('commit', '-m', 'Prepare isolated ten-game diagnostic', cwd=target)
    print(json.dumps({'project': str(target), 'baseline': git('rev-parse', 'HEAD', cwd=target), 'source_baseline': git('rev-parse', 'HEAD', cwd=source)}))


if __name__ == '__main__':
    main()
