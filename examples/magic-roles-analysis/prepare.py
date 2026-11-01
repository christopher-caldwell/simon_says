#!/usr/bin/env python3
"""Prepare the analysis-only continuation of a stopped ten-game diagnostic."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

HERE = Path(__file__).resolve().parent


def digest(data):
    return hashlib.sha256(data).hexdigest()


def read_json(path):
    return json.loads(path.read_text())


def capture(root):
    result = {}
    for f in sorted(root.rglob('*')):
        if f.is_symlink():
            raise ValueError('evidence contains a symlink: ' + str(f))
        if f.is_file():
            result[str(f.relative_to(root))] = digest(f.read_bytes())
    return result


def git(*args, cwd=None):
    return subprocess.run(['git', *map(str, args)], cwd=cwd, text=True, check=True, capture_output=True).stdout.strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('ten_game_project', type=Path)
    parser.add_argument('stopped_run', type=Path)
    parser.add_argument('new_project', type=Path)
    args = parser.parse_args()
    source = args.ten_game_project.resolve(strict=True)
    runroot = args.stopped_run.resolve(strict=True)
    target = args.new_project.resolve(strict=False)
    if target.exists() or target.is_relative_to(source) or target.is_relative_to(runroot):
        raise ValueError('target must be new and outside both inputs')
    if git('status', '--porcelain=v1', cwd=source):
        raise ValueError('source project must be clean')
    state_bytes = (runroot / 'state.json').read_bytes()
    state = json.loads(state_bytes)
    if state['state']['status'] != 'stopped' or state['state']['stage'] != 'supervise':
        raise ValueError('expected a stopped supervise-stage diagnostic')
    packet = read_json(runroot / 'actions/action-2/packet.json')
    if packet['stage'] != 'supervise' or packet['gate'] != 'worker':
        raise ValueError('action-2 is not the supervisor worker')
    store = (runroot / 'actions/action-2/scratch/native-store').resolve(strict=True)
    runs = list((store / 'runs').iterdir())
    if len(runs) != 1 or not runs[0].is_dir() or runs[0].is_symlink():
        raise ValueError('expected exactly one native run')
    run = runs[0].name
    files = capture(store)
    native = read_json(runs[0] / 'analysis.json')
    denominator = native['denominators']
    if (native['run'] != run or native['state'] != 'complete' or native['rulesCertified'] is not False
        or denominator['assignedGames'] != 10 or denominator['retainedAttempts'] != 10
        or denominator['eligibleGames'] != 6 or denominator['trialStates'] != {'excluded': 4, 'accepted': 6}):
        raise ValueError('native result is not the expected 10 assigned / 6 eligible / 4 excluded case')
    design = read_json(runroot / 'worktree/design.json')
    question = (source / 'original-task.txt').read_text()
    if design['original_question'] != question or design['planned_games'] != 10 or native['question'] != question.rstrip('\n'):
        raise ValueError('design and native question do not bind the source task')
    issues = [{'kind': 'exclusion', 'details': native['quality']['exclusionReasons']},
              {'kind': 'eligible_warning', 'details': native['quality']['eligibleQualityFlags']}]
    if issues[0]['details'] != {'unsupported_engine_behavior': 4}:
        raise ValueError('unexpected exclusion inventory; review before analysis')
    subprocess.run(['git', 'clone', '--no-hardlinks', '--', str(source), str(target)], check=True)
    for name in ('workflow.toml', 'analyze.md', 'native_tool.py', 'review.py'):
        shutil.copyfile(HERE / name, target / name)
    workflow = (target / 'workflow.toml').read_text().replace('"evidence-manifest.json",', '"evidence-manifest.json", "provenance.json",')
    (target / 'workflow.toml').write_text(workflow)
    def write(name, obj):
        (target / name).write_text(json.dumps(obj, indent=2, sort_keys=True) + '\n')
    write('evidence-manifest.json', {'store': str(store), 'run': run, 'files': files})
    write('provenance.json', {'source_project': str(source), 'source_commit': git('rev-parse', 'HEAD', cwd=source),
                              'stopped_run': str(runroot), 'stopped_state_sha256': digest(state_bytes)})
    collection = {'original_question': question, 'run': run, 'store': str(store), 'assigned': 10,
                  'attempts': 10, 'eligible': 6, 'excluded': issues[0]['details'], 'rules_certified': False,
                  'issues': issues, 'runtime_identity': native['runtimeIdentity']}
    write('collection.json', collection)
    write('design.json', design)
    schema = read_json(target / 'analysis.schema.json')
    schema['properties']['verified_games']['const'] = 6
    write('analysis.schema.json', schema)
    if files != capture(store) or state_bytes != (runroot / 'state.json').read_bytes():
        raise ValueError('original evidence or stopped run changed during preparation')
    git('add', '--all', cwd=target)
    git('commit', '-m', 'Prepare independent analysis of retained diagnostic', cwd=target)
    print(json.dumps({'project': str(target), 'baseline': git('rev-parse', 'HEAD', cwd=target),
                      'native_run': run, 'evidence_files': len(files)}))


if __name__ == '__main__':
    main()
