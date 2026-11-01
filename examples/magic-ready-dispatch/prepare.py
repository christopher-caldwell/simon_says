#!/usr/bin/env python3
"""Move long native collection to an independently reviewed controller command stage."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys


def git(repo, *args):
    return subprocess.run(['git', '-C', str(repo), *args], check=True, text=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout.strip()


def once(text, old, new, label):
    if text.count(old) != 1:
        raise ValueError(f'unexpected upstream {label}; do not silently adapt')
    return text.replace(old, new, 1)


def add_protected_plan(text):
    # Every subsequent stage must receive and preserve the exact reviewed plan.
    stage_names = {'collect', 'analyze', 'repair', 'build', 'simulate', 'triage', 'escalate',
                   'repair_origin', 'build_origin', 'simulate_origin', 'triage_final', 'escalate_final'}
    lines = text.splitlines()
    stage = None
    touched_inputs, touched_protection = set(), set()
    for index, line in enumerate(lines):
        if line.startswith('['):
            import re
            match = re.fullmatch(r'\[stages\.([a-z_]+)\]', line)
            stage = match.group(1) if match else None
        if stage in stage_names and line.startswith(('inputs = ', 'protected_inputs = ')):
            key, value = line.split(' = ', 1)
            items = json.loads(value)
            if 'collection-plan.json' in items:
                raise ValueError('duplicate plan declaration')
            items.append('collection-plan.json')
            lines[index] = key + ' = ' + json.dumps(items)
            (touched_inputs if key == 'inputs' else touched_protection).add(stage)
    if touched_inputs != stage_names or touched_protection != stage_names:
        raise ValueError('missing downstream input or protection declaration')
    return '\n'.join(lines) + '\n'


def add_protected_evidence(text):
    stages = {'triage', 'escalate', 'repair_origin', 'build_origin', 'simulate_origin', 'triage_final', 'escalate_final'}
    lines = text.splitlines()
    stage = None
    inputs, protection = set(), set()
    for index, line in enumerate(lines):
        if line.startswith('['):
            import re
            match = re.fullmatch(r'\[stages\.([a-z_]+)\]', line)
            stage = match.group(1) if match else None
        if stage in stages and line.startswith(('inputs = ', 'protected_inputs = ')):
            key, value = line.split(' = ', 1)
            values = json.loads(value)
            if 'origin-evidence.json' in values:
                raise ValueError('duplicate origin evidence declaration')
            values.append('origin-evidence.json')
            lines[index] = key + ' = ' + json.dumps(values)
            (inputs if key == 'inputs' else protection).add(stage)
    if inputs != stages or protection != stages:
        raise ValueError('missing downstream origin evidence input or protection')
    return '\n'.join(lines) + '\n'


def adapt_game(text):
    text = once(text, "packet['stage'] not in ('supervise', 'simulate', 'simulate_origin')",
                "packet['stage'] not in ('collect', 'simulate', 'simulate_origin')", 'game stage')
    old = """    root, files = frozen_files()
    source, cli, env = tool_source(files)
"""
    new = """    if packet['stage'] == 'collect':
        plan, design = load('collection-plan.json'), load('design.json')
        config_hash = sha(regular('simon-game/game-config.json'))
        if (plan['original_question'] != packet.get('run_context') or design['original_question'] != packet.get('run_context')
                or plan['config_sha256'] != config_hash or design['config_sha256'] != config_hash
                or plan['assigned_games'] != 10 or plan['launch_policy'] != 'once_no_retries'):
            raise ValueError('reviewed collection plan is inconsistent; no game may launch')
    root, files = frozen_files()
    source, cli, env = tool_source(files)
"""
    return once(text, old, new, 'prelaunch plan check')


def adapt_review(text):
    text = once(text, "'investigate': 'design.json', 'supervise': 'game.json',",
                "'investigate': 'design.json', 'supervise': 'collection-plan.json', 'collect': 'game.json',",
                'review allowed files')
    text = once(text, "if p['stage'] in ('supervise', 'repair', 'build', 'simulate', 'build_origin', 'simulate_origin'):",
                "if p['stage'] in ('collect', 'repair', 'build', 'simulate', 'build_origin', 'simulate_origin'):",
                'review delegation')
    needle = "elif p['stage'] == 'analyze':"
    branch = """elif p['stage'] == 'supervise':
    plan = json.loads(Path('collection-plan.json').read_text())
    design = json.loads(Path('design.json').read_text())
    config_hash = hashlib.sha256(Path('simon-game/game-config.json').read_bytes()).hexdigest()
    if (plan['original_question'] != p['run_context'] or design['original_question'] != p['run_context']
            or plan['config_sha256'] != config_hash or design['config_sha256'] != config_hash
            or plan['assigned_games'] != 10 or plan['launch_policy'] != 'once_no_retries'):
        result.update(outcome='changes_required', report='Preserve the exact question, config hash, ten-game bound and one-launch policy.')
    else:
        result['report'] = 'Reviewed the exact collection plan and frozen diagnostic configuration.'
"""
    text = once(text, needle, branch + needle, 'collection plan review')
    return text


def adapt_probe_review(text):
    text = once(text, "'analyze': 'analysis.json', 'triage': 'triage.json',",
                "'analyze': 'analysis.json', 'probe_origin': 'origin-evidence.json', 'triage': 'triage.json',", 'probe allowed files')
    marker = "elif p['stage'] == 'triage':"
    branch = """elif p['stage'] == 'probe_origin':
    probe = subprocess.run(['python3', '-B', 'simon-four/origin_probe.py', 'inspect'], text=True, capture_output=True, timeout=1750)
    if probe.returncode:
        raise ValueError('independent Origin probe failed: ' + probe.stderr[-2000:])
    if json.loads(Path('origin-evidence.json').read_text()) != json.loads(probe.stdout):
        result.update(outcome='changes_required', report='The evidence must match the independent frozen probe.')
    else:
        result['report'] = 'Independently reran the frozen probe on the exact candidate; live causality remains unproven.'
"""
    text = once(text, marker, branch + marker, 'probe review branch')
    old = """        probe = subprocess.run(['python3', '-B', 'simon-four/origin_probe.py', 'inspect'], text=True, capture_output=True, timeout=1750)
        if probe.returncode:
            raise ValueError('origin probe failed; no repair route: ' + probe.stderr[-2000:])
        evidence = json.loads(probe.stdout)
"""
    return once(text, old, "        evidence = json.loads(Path('origin-evidence.json').read_text())\n", 'reviewed probe input')

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('prepared_game_project', type=Path)
    parser.add_argument('collection_repo', type=Path)
    parser.add_argument('new_project', type=Path)
    parser.add_argument('--seed', help='optional distinct diagnostic namespace')
    args = parser.parse_args()
    here = Path(__file__).resolve().parent
    command = [sys.executable, '-B', str(here.parent / 'magic-ready-origin/prepare.py'),
               str(args.prepared_game_project), str(args.collection_repo), str(args.new_project)]
    if args.seed is not None:
        command += ['--seed', args.seed]
    subprocess.run(command, check=True)
    root = args.new_project.resolve(strict=True)
    for name in ('supervise.md', 'plan.schema.json', 'collect.md', 'probe-stage.py', 'probe-stage.md'):
        shutil.copyfile(here / name, root / 'simon-four' / name)

    workflow = root / 'workflow.toml'
    text = workflow.read_text()
    text = once(text, 'max_stage_transitions = 15', 'max_stage_transitions = 16', 'transition bound')
    text = once(text, '"simon-four/supervise.md",', '"simon-four/supervise.md", "simon-four/plan.schema.json", "simon-four/collect.md",', 'frozen files')
    start = text.index('[stages.supervise]'); end = text.index('\n[stages.analyze]', start)
    replacement = '''[stages.supervise]
worker = \"supervisor\"
reviewer = \"verifier\"
unblocker = \"stop\"
instructions = \"simon-four/supervise.md\"
inputs = [\"design.json\", \"simon-game/game-config.json\"]
protected_inputs = [\"design.json\"]
outputs = [\"collection-plan.json\"]
output_schema = \"simon-four/plan.schema.json\"
on_pass = \"collect\"
max_review_cycles = 2

[stages.collect]
worker = \"simulator\"
reviewer = \"verifier\"
unblocker = \"stop\"
instructions = \"simon-four/collect.md\"
inputs = [\"design.json\", \"simon-game/game-config.json\", \"decks/angels/checkpoint_2/deck_list.csv\", \"decks/angels/checkpoint_2/GUIDE.md\", \"decks/miirym/checkpoint_2/deck_list.csv\", \"decks/miirym/checkpoint_2/GUIDE.md\", \"decks/azula/checkpoint_1/deck_list.csv\", \"decks/azula/checkpoint_1/GUIDE.md\", \"decks/aang/checkpoint_1/deck_list.csv\", \"decks/aang/checkpoint_1/GUIDE.md\"]
protected_inputs = [\"design.json\"]
outputs = [\"game.json\"]
output_schema = \"simon-game/game.schema.json\"
on_pass = \"analyze\"
max_review_cycles = 2
[stages.collect.routes]
issues = \"analyze\"
clean = \"analyze\"
'''
    text = text[:start] + replacement + text[end:]
    text = add_protected_plan(text)
    text = once(text, 'max_stage_transitions = 16', 'max_stage_transitions = 17', 'probe transition bound')
    text = once(text, 'timeout_seconds = 1800\n[roles.verifier]',
                'timeout_seconds = 1800\n[roles.fixer.scratch_env]\nCARGO_TARGET_DIR = "cargo-target"\n[roles.verifier]', 'Fixer action-owned Cargo scratch')
    text = once(text, '"simon-four/collect.md",', '"simon-four/collect.md", "simon-four/probe-stage.py", "simon-four/probe-stage.md",', 'frozen probe files')
    text = once(text, '[stages.investigate]', '[roles.origin_probe]\nkind = "command"\nargv = ["python3", "-B", "simon-four/probe-stage.py"]\ntimeout_seconds = 1800\n\n[stages.investigate]', 'probe role')
    text = once(text, 'warnings = "triage"', 'warnings = "probe_origin"', 'probe route')
    stage = '''[stages.probe_origin]
worker = "origin_probe"
reviewer = "verifier"
unblocker = "stop"
instructions = "simon-four/probe-stage.md"
inputs = ["design.json", "game.json", "analysis.json", "repair.json", "native-runtime.json", "verification.json", "collection-plan.json"]
protected_inputs = ["design.json", "game.json", "analysis.json", "repair.json", "native-runtime.json", "verification.json", "collection-plan.json"]
outputs = ["origin-evidence.json"]
on_pass = "triage"
max_review_cycles = 2

'''
    text = once(text, '[stages.triage]', stage + '[stages.triage]', 'probe stage')
    workflow.write_text(add_protected_evidence(text))

    triage = root / 'simon-four/triage.md'
    text = triage.read_text()
    text = once(text, 'then independently run `python3 -B simon-four/verify.py verify-post` once and `python3 -B simon-four/origin_probe.py inspect` once in action scratch.',
                'and the independently reviewed origin-evidence.json, then independently run `python3 -B simon-four/verify.py verify-post` once in action scratch. Do not rerun the long Origin probe.', 'triage probe instruction')
    triage.write_text(text)
    for name in ('fix.md', 'fix-origin.md'):
        instructions = root / 'simon-four' / name
        instructions.write_text(instructions.read_text() + '\nThe controller sets CARGO_TARGET_DIR to this action’s scratch directory. Preserve it for all Cargo commands; do not create build outputs in the managed checkout.\n')

    profile = root / 'simon-four/profiles/supervisor.md'
    text = profile.read_text()
    old = 'Run the explicitly authorized isolated ten-game diagnostic through simon-game/game.py, verify structure and capture issue inventory.'
    new = 'Review the explicitly authorized isolated ten-game design and commit its collection plan; the controller directly launches and verifies the deterministic batch after review.'
    profile.write_text(once(text, old, new, 'supervisor role adaptation'))

    game = root / 'simon-game/game.py'
    game.write_text(adapt_game(game.read_text()))

    native_review = root / 'simon-game/review.py'
    text = native_review.read_text()
    native_review.write_text(once(text, "if p['stage'] in ('supervise', 'simulate', 'simulate_origin'):",
                                  "if p['stage'] in ('collect', 'simulate', 'simulate_origin'):", 'game review stage'))

    review = root / 'simon-four/review.py'
    review.write_text(adapt_probe_review(adapt_review(review.read_text())))
    git(root, 'add', '--', 'workflow.toml', 'simon-four', 'simon-game/game.py', 'simon-game/review.py')
    git(root, 'commit', '-m', 'Dispatch bounded collection directly after reviewed Supervisor handoff')
    print('Prepared directly supervised collection baseline ' + git(root, 'rev-parse', 'HEAD'))


if __name__ == '__main__':
    main()
