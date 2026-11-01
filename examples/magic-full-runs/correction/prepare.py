#!/usr/bin/env python3
"""Prepare an induced, evidence-bound review correction without collecting games."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

HERE = Path(__file__).resolve().parent


def git(*args, cwd):
    return subprocess.run(['git', *map(str,args)], cwd=cwd, text=True, check=True, capture_output=True).stdout.strip()


def capture(root):
    files = {}
    for path in sorted(root.rglob('*')):
        if path.is_symlink():
            raise ValueError(f'evidence contains symlink: {path}')
        if path.is_file():
            files[str(path.relative_to(root))] = hashlib.sha256(path.read_bytes()).hexdigest()
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source_project', type=Path)
    parser.add_argument('completed_run', type=Path)
    parser.add_argument('new_project', type=Path)
    args = parser.parse_args()
    source = args.source_project.resolve(strict=True)
    runroot = args.completed_run.resolve(strict=True)
    target = args.new_project.resolve(strict=False)
    if target.exists() or target.is_relative_to(source) or target.is_relative_to(runroot):
        raise ValueError('destination must be new and outside source and run')
    if git('status', '--porcelain=v1', cwd=source):
        raise ValueError('source project must be clean')
    statebytes = (runroot/'state.json').read_bytes()
    state = json.loads(statebytes)
    if state['state']['status'] != 'awaiting_approval' or Path(state['worktree']['source']).resolve() != source:
        raise ValueError('expected source-bound run awaiting approval')
    worktree = runroot/'worktree'
    if git('status', '--porcelain=v1', cwd=worktree) or git('rev-parse', 'HEAD', cwd=worktree) != state['state']['accepted_commit']:
        raise ValueError('accepted worktree is dirty or at wrong commit')
    collection = json.loads((worktree/'collection.json').read_text())
    store = Path(collection['store']).resolve(strict=True)
    if not store.is_relative_to((runroot/'actions').resolve(strict=True)):
        raise ValueError('store is outside completed run actions')
    run = collection['run']
    if not isinstance(run, str) or '/' in run or run in ('', '.', '..'):
        raise ValueError('invalid native run')
    files = capture(store)
    subprocess.run(['git','clone','--no-hardlinks','--',str(source),str(target)], check=True)
    for name in ('workflow.toml','analyze.md','review.py','native_tool.py'):
        shutil.copyfile(HERE/name, target/name)
    for name in ('design.json','collection.json'):
        shutil.copyfile(worktree/name, target/name)
    schema_path = target/'analysis.schema.json'
    schema = json.loads(schema_path.read_text())
    schema['properties']['excluded_count'] = {'type':'integer','minimum':0,'maximum':4}
    schema_path.write_text(json.dumps(schema) + '\n')
    (target/'evidence-manifest.json').write_text(json.dumps({'store':str(store),'run':run,'files':files},indent=2,sort_keys=True)+'\n')
    if files != capture(store) or statebytes != (runroot/'state.json').read_bytes():
        raise ValueError('source evidence or run state changed during preparation')
    git('add','--all',cwd=target)
    git('commit','-m','Prepare induced evidence-bound correction exercise',cwd=target)
    print(json.dumps({'project':str(target),'baseline':git('rev-parse','HEAD',cwd=target),'files':len(files)}))


if __name__ == '__main__':
    main()
