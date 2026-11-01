#!/usr/bin/env python3
"""Prepare isolated real four-role diagnostic from prepared native-game baseline."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib

QUESTION = ('Can this isolated native runtime produce ten verified eligible diagnostic games, '
            'what does that evidence establish, and what scoped engine repair is warranted?')


def git(repo, *args):
    return subprocess.run(['git', '-C', str(repo), *args], check=True, text=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout.strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('prepared_game_project', type=Path)
    parser.add_argument('collection_repo', type=Path)
    parser.add_argument('new_project', type=Path)
    args = parser.parse_args()
    here = Path(__file__).resolve().parent
    source = args.prepared_game_project.resolve(strict=True)
    collection = args.collection_repo.resolve(strict=True)
    requested = args.new_project.absolute()
    target = requested.parent.resolve(strict=True) / requested.name
    if target.exists() or target.is_symlink() or target.is_relative_to(source) or target.is_relative_to(collection):
        raise ValueError('destination must not exist and must be outside sources')
    if git(source, 'status', '--porcelain=v1'):
        raise ValueError('prepared game project must be clean')
    base = git(source, 'rev-parse', 'HEAD')
    config = json.loads((source / 'simon-game/game-config.json').read_text())
    if config.get('games') != 10 or config.get('seedNamespace') != 'simon-says-ten-game-diagnostic-20261101':
        raise ValueError('unexpected prepared game configuration')
    runtime = Path(json.loads((source / 'simon-game/assets.json').read_text())['root']).resolve(strict=True)
    # Capture current real role TOMLs/guidance through the existing provenance-aware preparer.
    with tempfile.TemporaryDirectory(prefix='simon-four-profiles-') as temp:
        profile_project = Path(temp) / 'roles'
        subprocess.run([sys.executable, '-B', str(here.parent / 'magic-roles/prepare.py'),
                        str(collection), str(runtime), str(profile_project)], check=True)
        subprocess.run(['git', 'clone', '--no-hardlinks', '--no-checkout', '--', str(source), str(target)], check=True)
        git(target, 'checkout', '--detach', base)
        git(target, 'config', 'user.name', 'Simon Says Diagnostic')
        git(target, 'config', 'user.email', 'simon-diagnostic@example.invalid')
        dest = target / 'simon-four'
        dest.mkdir()
        for name in ['investigate.md', 'supervise.md', 'analyze.md', 'fix.md', 'design.schema.json',
                     'analysis.schema.json', 'verify.py', 'review.py']:
            shutil.copyfile(here / name, dest / name)
        (dest / 'profiles').mkdir()
        overrides = {
            'supervisor': ('Run the one explicitly authorized isolated diagnostic through native_tool.py, verify structure and capture issue inventory.',
                           'Run the explicitly authorized isolated ten-game diagnostic through simon-game/game.py, verify structure and capture issue inventory.'),
            'fixer': ('Configured for a separate explicitly requested repair scope only. This diagnostic never routes to Fixer, authorizes no repair and does not change the engine.',
                      'The user explicitly requested this diagnostic Fixer scope. Repair only the independently reviewed reported issue in the isolated engine worktree; do not run games or install a runtime.'),
        }
        roles = tomllib.loads((profile_project / 'workflow.toml').read_text())['roles']
        workflow = (here / 'workflow.toml').read_text()
        for role in ['investigator', 'supervisor', 'analyzer', 'fixer']:
            content = (profile_project / 'profiles' / (role + '.md')).read_text()
            if role in overrides:
                old, new = overrides[role]
                if content.count(old) != 1:
                    raise ValueError('unexpected profile adaptation for ' + role)
                content = content.replace(old, new, 1)
            (dest / 'profiles' / (role + '.md')).write_text(content)
            # Bind exact provider settings captured from current real role TOML.
            a = workflow.index('[roles.' + role + ']')
            b = workflow.find('\n[', a + 1)
            chunk = workflow[a:b if b >= 0 else None]
            import re
            chunk = re.sub(r'(?m)^model = .*$', 'model = ' + json.dumps(roles[role]['model']), chunk)
            chunk = re.sub(r'(?m)^effort = .*$', 'effort = ' + json.dumps(roles[role]['effort']), chunk)
            workflow = workflow[:a] + chunk + (workflow[b:] if b >= 0 else '')
        (target / 'workflow.toml').write_text(workflow)
    game = target / 'simon-game/game.py'
    text = game.read_text()
    old = "packet['stage'] != 'simulate'"
    if text.count(old) != 1:
        raise ValueError('unexpected native helper stage check')
    game.write_text(text.replace(old, "packet['stage'] != 'supervise'"))
    review = target / 'simon-game/review.py'
    text = review.read_text()
    old = "p['stage'] == 'simulate'"
    if text.count(old) != 1:
        raise ValueError('unexpected native reviewer stage check')
    review.write_text(text.replace(old, "p['stage'] == 'supervise'"))
    (target / 'original-task.txt').write_text(QUESTION)
    git(target, 'add', '--', 'workflow.toml', 'simon-four', 'simon-game/game.py', 'simon-game/review.py', 'original-task.txt')
    git(target, 'commit', '-m', 'Prepare four-role native diagnostic')
    print(json.dumps({'project': str(target), 'baseline': git(target, 'rev-parse', 'HEAD'),
                      'game_baseline': base, 'runtime': str(runtime), 'question': QUESTION}))


if __name__ == '__main__':
    main()
