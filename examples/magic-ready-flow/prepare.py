#!/usr/bin/env python3
"""Prepare the bounded four-role, repair, rebuild and warning-triage diagnostic."""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib


def git(repo, *args):
    return subprocess.run(['git', '-C', str(repo), *args], check=True, text=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout.strip()


def replace_once(text, old, new, description):
    if text.count(old) != 1:
        raise ValueError(f'unexpected upstream {description}; do not silently adapt')
    return text.replace(old, new, 1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('prepared_game_project', type=Path)
    parser.add_argument('collection_repo', type=Path)
    parser.add_argument('new_project', type=Path)
    args = parser.parse_args()
    here = Path(__file__).resolve().parent
    target = args.new_project.absolute()
    # The existing preparer enforces clean source, absent isolated destination,
    # native/runtime provenance, and actual profile/model/effort snapshots.
    subprocess.run([sys.executable, '-B', str(here.parent / 'magic-four-roles/prepare.py'),
                    str(args.prepared_game_project), str(args.collection_repo), str(target)], check=True)
    target = target.resolve(strict=True)
    old_workflow = tomllib.loads((target / 'workflow.toml').read_text())
    workflow = (here / 'workflow.toml').read_text()
    for role in ('investigator', 'supervisor', 'analyzer', 'fixer'):
        start = workflow.index('[roles.' + role + ']')
        end = workflow.find('\n[', start + 1)
        chunk = workflow[start:end if end >= 0 else None]
        for field in ('model', 'effort'):
            chunk, count = re.subn(r'(?m)^' + field + r' = .*$',
                                   field + ' = ' + json.dumps(old_workflow['roles'][role][field]), chunk)
            if count != 1:
                raise ValueError(f'missing {role} {field} in fixture')
        workflow = workflow[:start] + chunk + (workflow[end:] if end >= 0 else '')
    (target / 'workflow.toml').write_text(workflow)
    for name in ('triage.md', 'triage.schema.json', 'verify-stage.md', 'build-stage.md', 'review.py', 'verify.py', 'escalate.py'):
        shutil.copyfile(here / name, target / 'simon-four' / name)
    game = target / 'simon-game/game.py'
    source = game.read_text()
    source = replace_once(source, "packet['stage'] != 'supervise'", "packet['stage'] not in ('supervise', 'simulate')", 'game stage guard')
    source = replace_once(source, "Path('game.json').write_text(json.dumps(report, indent=2) + '\\n')\n    subprocess.run(['git', 'add', '--', 'game.json'], check=True)",
                          "output = 'verification.json' if packet['stage'] == 'simulate' else 'game.json'\n    Path(output).write_text(json.dumps(report, indent=2) + '\\n')\n    subprocess.run(['git', 'add', '--', output], check=True)", 'game output')
    game.write_text(source)
    review = target / 'simon-game/review.py'
    source = review.read_text()
    source = replace_once(source, "if p['stage'] == 'supervise':", "if p['stage'] in ('supervise', 'simulate'):", 'review stage guard')
    source = replace_once(source, "report = load('game.json')", "report = load('verification.json' if p['stage'] == 'simulate' else 'game.json')", 'review input')
    old = "response.update(route=report['kind'], report='Independently verified native game evidence; route: ' + report['kind'])"
    new = """route = report['kind']
        if p['stage'] == 'simulate' and (route == 'issues' or any(report['issues']['all_warnings'].values())):
            route = 'warnings'
        response.update(route=route, report='Independently verified native game evidence; route: ' + route)"""
    source = replace_once(source, old, new, 'review warning route')
    review.write_text(source)
    git(target, 'add', '--', 'workflow.toml', 'simon-four', 'simon-game/game.py', 'simon-game/review.py')
    git(target, 'commit', '-m', 'Add build, verification and warning triage')
    print(json.dumps({'project': str(target), 'baseline': git(target, 'rev-parse', 'HEAD')}))


if __name__ == '__main__':
    main()
