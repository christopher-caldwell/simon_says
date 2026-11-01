#!/usr/bin/env python3
"""Deterministic roles for a structured expense-normalization workflow."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys


def read(path):
    return json.loads(Path(path).read_text())


def git(*args):
    return subprocess.check_output(['git', *args], text=True).strip()


def measure(records):
    if not isinstance(records, list):
        raise ValueError('records must be an array')
    total = dirty = 0
    ids = set()
    for item in records:
        if not isinstance(item, dict) or set(item) != {'id', 'amount'}:
            raise ValueError('each record requires exactly id and amount')
        ident, amount = item['id'], item['amount']
        if not isinstance(ident, str) or not ident or ident in ids or not isinstance(amount, str):
            raise ValueError('invalid or duplicate record identity or amount')
        ids.add(ident)
        value = amount.strip()
        if not re.fullmatch(r'[0-9]+\.[0-9]{2}', value):
            raise ValueError('amount must be a nonnegative decimal with two fractional digits')
        whole, fraction = value.split('.')
        total += int(whole) * 100 + int(fraction)
        dirty += value != amount
    return {'count': len(records), 'total_cents': total, 'needs_cleanup': dirty}


def commit(*paths):
    git('add', '--', *paths)
    git('-c', 'commit.gpgsign=false', 'commit', '--allow-empty', '-m', 'Record workflow result')
    return git('rev-parse', 'HEAD')


def respond(packet, outcome, report, candidate=None, inspected=None, route=None):
    result = {'action_id': packet['action_id'], 'outcome': outcome, 'report': report,
              'candidate_commit': candidate, 'inspected_commit': inspected}
    if route is not None:
        result['route'] = route
    print(json.dumps(result))


def main():
    packet = read(os.environ['SIMON_PACKET'])
    role = sys.argv[1]
    if role == 'stop':
        respond(packet, 'blocked', 'Input cannot be safely normalized without a decision.')
        return
    if role == 'auditor':
        try:
            report = measure(read('records.json'))
        except (ValueError, TypeError) as error:
            respond(packet, 'blocked', str(error))
            return
        Path('audit.json').write_text(json.dumps(report) + '\n')
        respond(packet, 'complete', 'Audited records.', candidate=commit('audit.json'))
    elif role == 'normalizer':
        records = read('records.json')
        report = measure(records)
        if report != read('audit.json') or not report['needs_cleanup']:
            raise ValueError('reviewed audit does not authorize normalization')
        for item in records:
            item['amount'] = item['amount'].strip()
        Path('records.json').write_text(json.dumps(records) + '\n')
        Path('repair.json').write_text(json.dumps({'normalized': report['needs_cleanup']}) + '\n')
        respond(packet, 'complete', 'Normalized whitespace only.', candidate=commit('records.json', 'repair.json'))
    elif role == 'checker':
        head = git('rev-parse', 'HEAD')
        if head != packet['candidate_commit'] or packet['gate'] != 'reviewer':
            raise ValueError('review candidate identity mismatch')
        report = measure(read('records.json'))
        if packet['stage'] == 'audit':
            if report != read('audit.json'):
                respond(packet, 'changes_required', 'Correct the audit.', inspected=head)
            else:
                route = 'needs_cleanup' if report['needs_cleanup'] else 'done'
                respond(packet, 'pass', 'Verified audit and selected declared route.', inspected=head, route=route)
        elif packet['stage'] == 'normalize':
            before = json.loads(git('show', packet['baseline_commit'] + ':records.json'))
            expected = [{'id': item['id'], 'amount': item['amount'].strip()} for item in before]
            if (read('records.json') != expected or report['needs_cleanup']
                    or read('repair.json')['normalized'] != read('audit.json')['needs_cleanup']):
                respond(packet, 'changes_required', 'Correct the normalization without changing record values.', inspected=head)
            else:
                respond(packet, 'pass', 'Verified normalization against the preceding checkpoint.', inspected=head)
        else:
            raise ValueError('unknown stage')
    else:
        raise ValueError('unknown role')


if __name__ == '__main__':
    main()
