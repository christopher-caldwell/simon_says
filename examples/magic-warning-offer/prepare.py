#!/usr/bin/env python3
"""Prepare a disposable observed-warning repair project from the recorded engine candidate."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tomllib

SOURCE_COMMIT = '97bfa479d17dcd494cb9196805f5650d92072d96'
MARKER = '\n#[cfg(test)]\n#[path = "game_loop/origin_offer_regression.rs"]\nmod origin_offer_regression;\n'


def git(*args, cwd=None):
    return subprocess.run(['git', *map(str, args)], cwd=cwd, check=True, text=True,
                          stdout=subprocess.PIPE).stdout.strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('engine_repo', type=Path, help='read-only local repository containing recorded candidate commit')
    parser.add_argument('collection_repo', type=Path, help='read-only Magic role source')
    parser.add_argument('new_project', type=Path, help='new isolated checkout, outside source repositories')
    args = parser.parse_args()
    engine = Path(git('-C', args.engine_repo.resolve(strict=True), 'rev-parse', '--show-toplevel'))
    collection = args.collection_repo.resolve(strict=True)
    requested = args.new_project.absolute()
    target = requested.parent.resolve(strict=True) / requested.name
    example_repo = Path(__file__).resolve().parents[2]
    if target.exists() or target.is_symlink() or any(target.is_relative_to(p) for p in (engine, collection, example_repo)):
        raise ValueError('destination must be new and outside the source and Simon Says repositories')
    if git('-C', engine, 'rev-parse', '--verify', SOURCE_COMMIT + '^{commit}') != SOURCE_COMMIT:
        raise ValueError('recorded baseline candidate missing')
    role_path = collection / '.codex/agents/fixer.toml'
    raw = role_path.read_bytes()
    role = tomllib.loads(raw.decode())
    git('clone', '--no-hardlinks', '--no-checkout', '--', engine, target)
    git('checkout', '--detach', SOURCE_COMMIT, cwd=target)
    git('config', 'user.name', 'Simon Says Isolated Fixture', cwd=target)
    git('config', 'user.email', 'simon-fixture@example.invalid', cwd=target)
    if git('ls-files', '--stage', '--', 'forge', cwd=target).startswith('160000 '):
        raise ValueError('recorded candidate unexpectedly contains unsupported gitlink')
    templates = Path(__file__).resolve().parent
    dest = target / 'simon-warning'
    dest.mkdir()
    for name in ('fix.md', 'issue.json', 'schema.json', 'review.py', 'stop.py'):
        shutil.copyfile(templates / name, dest / name)
    role_dest = target / 'simon-repair/profile.md'
    role_dest.parent.mkdir(parents=True, exist_ok=True)
    role_dest.write_text(
        '# Scoped Fixer role\n\nYour only authority is the explicit isolated request in simon-warning/fix.md. '
        'Historical instructions for live source, installation, no commits, dispatch or publication are '
        'superseded solely for this disposable checkpoint. Never access other checkouts or alter evidence.\n\n'
        f'Source profile SHA256 {hashlib.sha256(raw).hexdigest()}\n\n```toml\n{raw.decode()}\n```\n')
    module = target / 'manabrew-rs/crates/manabrew-engine/src/game_loop.rs'
    text = module.read_text()
    if 'mod origin_offer_regression;' in text:
        raise ValueError('test module already exists in candidate')
    module.write_text(text + MARKER)
    shutil.copyfile(templates / 'origin_offer_regression.rs', module.parent / 'game_loop/origin_offer_regression.rs')
    workflow = (templates / 'workflow.toml').read_text()
    workflow = workflow.replace('model = "gpt-6.1-terra"', 'model = ' + json.dumps(role['model']))
    workflow = workflow.replace('effort = "high"', 'effort = ' + json.dumps(role['model_reasoning_effort']))
    (target / 'warning-workflow.toml').write_text(workflow)
    if role_path.read_bytes() != raw:
        raise RuntimeError('source role changed during preparation; inspect incomplete clone')
    git('add', 'warning-workflow.toml', 'simon-warning', 'simon-repair/profile.md',
        'manabrew-rs/crates/manabrew-engine/src/game_loop.rs',
        'manabrew-rs/crates/manabrew-engine/src/game_loop/origin_offer_regression.rs', cwd=target)
    git('commit', '-m', 'Prepare observed modal action space repair controls', cwd=target)
    print(json.dumps({'project': str(target), 'source_commit': SOURCE_COMMIT,
                      'baseline': git('rev-parse', 'HEAD', cwd=target),
                      'profile_sha256': hashlib.sha256(raw).hexdigest(),
                      'model': role['model'], 'effort': role['model_reasoning_effort']}))


if __name__ == '__main__':
    main()
