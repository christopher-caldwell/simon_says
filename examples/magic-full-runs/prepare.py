#!/usr/bin/env python3
"""Prepare an isolated four-game role diagnostic from a clean ten-game fixture."""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess

HERE = Path(__file__).resolve().parent


def git(*args, cwd):
    return subprocess.run(['git', *map(str, args)], cwd=cwd, check=True, text=True, capture_output=True).stdout.strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('target', type=Path)
    parser.add_argument('--seed', required=True)
    args = parser.parse_args()
    if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_-]{0,79}', args.seed):
        raise ValueError('seed must be a simple bounded identifier')
    source = args.source.resolve(strict=True)
    target = args.target.resolve(strict=False)
    if target.exists() or target.is_relative_to(source) or git('status', '--porcelain=v1', cwd=source):
        raise ValueError('target must be new/outside source and source must be clean')
    subprocess.run(['git', 'clone', '--no-hardlinks', '--', str(source), str(target)], check=True)
    workflow_path = target / 'workflow.toml'
    workflow = workflow_path.read_text().replace('ten isolated native diagnostic games', 'four isolated native diagnostic games')
    if workflow == workflow_path.read_text():
        raise ValueError('expected ten-game workflow purpose missing; refuse drift')
    workflow_path.write_text(workflow)
    (target / 'README.md').write_text('Isolated four-game role-handoff diagnostic. See frozen workflow.toml and instructions. This is not a production study.\n')
    for name in ('native_tool.py', 'review.py', 'investigate.md', 'supervise.md', 'analyze.md'):
        shutil.copyfile(HERE / name, target / name)
    config = json.loads((target / 'diagnostic.json').read_text())
    config.update(games=4, workers=1, budgetSeconds=210, seedNamespace=args.seed,
                  question='What verified evidence and limitations result from this four-game diagnostic?')
    (target / 'diagnostic.json').write_text(json.dumps(config, indent=2) + '\n')
    (target / 'original-task.txt').write_text(config['question'] + '\n')
    for filename, properties in [('design.schema.json', {'planned_games': {'const': 4}}),
        ('collection.schema.json', {'assigned': {'const': 4}, 'attempts': {'const': 4}, 'eligible': {'type': 'integer', 'minimum': 0, 'maximum': 4}}),
        ('analysis.schema.json', {'verified_games': {'type': 'integer', 'minimum': 0, 'maximum': 4}})]:
        path = target / filename
        schema = json.loads(path.read_text())
        schema['properties'].update(properties)
        path.write_text(json.dumps(schema) + '\n')
    git('add', '--all', cwd=target)
    git('commit', '-m', 'Prepare isolated four-game diagnostic', cwd=target)
    print(json.dumps({'project': str(target), 'baseline': git('rev-parse', 'HEAD', cwd=target), 'seed': args.seed}))


if __name__ == '__main__':
    main()
