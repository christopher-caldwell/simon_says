#!/usr/bin/env python3
"""Prepare a bounded actual-archive Origin follow-up on the four-role diagnostic."""
import argparse
from pathlib import Path
import shutil
import subprocess
import sys


def git(repo, *args):
    return subprocess.run(['git', '-C', str(repo), *args], check=True, text=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout.strip()


def replace_once(text, old, new, description):
    if text.count(old) != 1:
        raise ValueError(f'unexpected upstream {description}; do not silently adapt')
    return text.replace(old, new, 1)


def transform_game_review(text):
    text = replace_once(text, "if p['stage'] in ('supervise', 'simulate'):",
                        "if p['stage'] in ('supervise', 'simulate', 'simulate_origin'):", 'game review stage')
    text = replace_once(text, "report = load('verification.json' if p['stage'] == 'simulate' else 'game.json')",
                        "report = load('verification-origin.json' if p['stage'] == 'simulate_origin' else 'verification.json' if p['stage'] == 'simulate' else 'game.json')", 'game review input')
    text = replace_once(text, "if Path('native-runtime.json').exists():\n        record = load('native-runtime.json')",
                        "runtime_path = 'native-runtime-origin.json' if p['stage'] == 'simulate_origin' else 'native-runtime.json'\n    if Path(runtime_path).exists():\n        record = load(runtime_path)", 'review runtime')
    text = replace_once(text, "if p['stage'] == 'simulate' and (route == 'issues' or any(report['issues']['all_warnings'].values())):",
                        "if p['stage'] in ('simulate', 'simulate_origin') and (route == 'issues' or any(report['issues']['all_warnings'].values())):", 'warning route')
    text = replace_once(text, "if report['kind'] == 'unresolved':", "if report['kind'] == 'unresolved' or (p['stage'] in ('simulate', 'simulate_origin') and report['kind'] == 'issues'):", 'fail safe persistent scoped issue')
    text = replace_once(text, "elif p['stage'] == 'build':\n    record = load('native-runtime.json')",
                        "elif p['stage'] in ('build', 'build_origin'):\n    record = load('native-runtime-origin.json' if p['stage'] == 'build_origin' else 'native-runtime.json')", 'build review stage')
    return text


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('prepared_game_project', type=Path)
    parser.add_argument('collection_repo', type=Path)
    parser.add_argument('new_project', type=Path)
    parser.add_argument('--seed', help='optional distinct diagnostic namespace')
    args = parser.parse_args()
    here = Path(__file__).resolve().parent
    command = [sys.executable, '-B', str(here.parent / 'magic-ready-flow/prepare.py'),
               str(args.prepared_game_project), str(args.collection_repo), str(args.new_project)]
    if args.seed is not None:
        command += ['--seed', args.seed]
    subprocess.run(command, check=True)
    root = args.new_project.resolve(strict=True)
    for name in ('origin_probe.py', 'origin_offer_regression.rs', 'triage.md', 'triage.schema.json',
                 'fix-origin.md', 'triage-final.md', 'triage-final.schema.json',
                 'warning-repair.schema.json', 'review.py'):
        shutil.copyfile(here / name, root / 'simon-four' / name)

    workflow = root / 'workflow.toml'
    text = workflow.read_text()
    text = replace_once(text, 'max_stage_transitions = 9', 'max_stage_transitions = 15', 'workflow limit')
    text = replace_once(text, '"simon-four/triage.md",',
                        '"simon-four/triage.md", "simon-four/origin_probe.py", "simon-four/origin_offer_regression.rs", "simon-four/fix-origin.md", "simon-four/triage-final.md", "simon-four/triage-final.schema.json", "simon-four/warning-repair.schema.json",',
                        'workflow frozen files')
    start = text.index('[stages.triage]'); end = text.index('\n[stages.escalate]', start)
    triage = text[start:end]
    triage = replace_once(triage, 'max_review_cycles = 2',
                          'max_review_cycles = 2\n[stages.triage.routes]\norigin = "repair_origin"\nunresolved = "escalate"',
                          'triage routes')
    text = text[:start] + triage + text[end:] + (here / 'extra-stages.toml').read_text()
    workflow.write_text(text)

    native = root / 'simon-game/native.py'
    text = native.read_text()
    old = """if (source_commit != packet['baseline_commit'] or transition.get('from_stage') != 'repair'
            or transition.get('to_stage') != 'build' or transition.get('reviewed_commit') != source_commit):"""
    new = """if (source_commit != packet['baseline_commit']
            or (transition.get('from_stage'), transition.get('to_stage')) not in {('repair', 'build'), ('repair_origin', 'build_origin')}
            or transition.get('to_stage') != packet['stage'] or transition.get('reviewed_commit') != source_commit):"""
    native.write_text(replace_once(text, old, new, 'runtime source binding'))

    build = root / 'simon-game/build.py'
    text = build.read_text()
    text = replace_once(text, "p['stage'] != 'build'", "p['stage'] not in ('build', 'build_origin')", 'build stage')
    text = replace_once(text,
        "Path('native-runtime.json').write_text(json.dumps(record, indent=2) + '\\n')\nsubprocess.run(['git', 'add', '--', 'native-runtime.json'], check=True)",
        "output = 'native-runtime-origin.json' if p['stage'] == 'build_origin' else 'native-runtime.json'\nPath(output).write_text(json.dumps(record, indent=2) + '\\n')\nsubprocess.run(['git', 'add', '--', output], check=True)", 'build output')
    build.write_text(text)

    game = root / 'simon-game/game.py'
    text = game.read_text()
    text = replace_once(text, "packet['stage'] not in ('supervise', 'simulate')",
                        "packet['stage'] not in ('supervise', 'simulate', 'simulate_origin')", 'game stage')
    text = replace_once(text, "record = Path('native-runtime.json')",
                        "record = Path('native-runtime-origin.json' if packet['stage'] == 'simulate_origin' else 'native-runtime.json')", 'game runtime')
    text = replace_once(text, "output = 'verification.json' if packet['stage'] == 'simulate' else 'game.json'",
                        "output = ('verification-origin.json' if packet['stage'] == 'simulate_origin' else 'verification.json' if packet['stage'] == 'simulate' else 'game.json')", 'game output')
    game.write_text(text)

    review = root / 'simon-game/review.py'
    review.write_text(transform_game_review(review.read_text()))

    verify = root / 'simon-four/verify.py'
    text = verify.read_text()
    text = replace_once(text, "if sys.argv[1:] not in (['verify'], ['verify-post']):",
                        "if sys.argv[1:] not in (['verify'], ['verify-post'], ['verify-origin']):", 'verify modes')
    text = replace_once(text, "report = load('verification.json' if sys.argv[1:] == ['verify-post'] else 'game.json')",
                        "report = load('verification-origin.json' if sys.argv[1:] == ['verify-origin'] else 'verification.json' if sys.argv[1:] == ['verify-post'] else 'game.json')", 'verify input')
    verify.write_text(text)
    escalate = root / 'simon-four/escalate.py'
    escalate.write_text(replace_once(escalate.read_text(), "p['stage'] != 'escalate'",
                                    "p['stage'] not in ('escalate', 'escalate_final')", 'escalation stage'))
    git(root, 'add', '--', 'workflow.toml', 'simon-four', 'simon-game/native.py',
        'simon-game/game.py', 'simon-game/build.py', 'simon-game/review.py')
    git(root, 'commit', '-m', 'Add bounded actual-archive Origin warning follow-up')
    print('Prepared origin project baseline ' + git(root, 'rev-parse', 'HEAD'))


if __name__ == '__main__':
    main()
