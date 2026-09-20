"""Validate retained lev-serve admission evidence offline; never calls a model."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate(capture, models, source):
    report = {'capture': str(capture), 'models': str(models), 'source': str(source),
              'verified': False, 'errors': []}
    errors = report['errors']
    def require(condition, message):
        if not condition:
            errors.append(message)
    try:
        text = source.read_text()
        secret = re.search(r'const SECRET: &str = "([^"]+)";', text).group(1)
        decoy_text = re.search(r'const DECOYS: \[&str; 3\] = \[([^]]+)\];', text).group(1)
        options = [secret] + re.findall(r'"([^"]+)"', decoy_text)
        require(len(options) == 4, 'source does not declare four probe options')
        rows = [json.loads(line) for line in capture.read_text().splitlines()]
        cards = json.loads(models.read_text())['models']
        require(len(cards) == 1, 'expected exactly one model card')
        isolation = cards[0]['isolation']
        require(isolation['seeds'] == 8, 'startup did not use eight probe seeds')
        report.update(capture_sha256=digest(capture), models_sha256=digest(models),
                      source_sha256=digest(source), rows=len(rows))
        require(len(rows) == 25, 'expected one planted and three eight-draw arms')
        expected_arms = ['planted'] + ['sibling'] * 8 + ['absent'] * 8 + ['state'] * 8
        require([row.get('arm') for row in rows] == expected_arms, 'probe arm ordering differs')
        ids = []
        hits = {arm: 0 for arm in ('sibling', 'absent', 'state')}
        for index, row in enumerate(rows):
            require(row.get('schema') == 'openagents.lev.admission_probe_row.v1', f'row {index}: schema differs')
            call = row['call']
            ids.append(call['id'])
            require(isinstance(call['id'], str) and bool(call['id']), f'row {index}: call ID missing')
            require(call['op'] == 'decide', f'row {index}: operation is not decide')
            require(isinstance(call['instructions'], str) and isinstance(call['prompt'], str), f'row {index}: call text missing')
            if index == 0:
                require(call['sampling'] == {'mode': 'greedy'}, 'planted call is not greedy')
                require(call['options'] == ['a', 'b'], 'planted options differ')
                require(secret in call['instructions'], 'planted instruction lacks the token')
            else:
                seed = (index - 1) % 8
                require(call['sampling'] == {'mode': 'random', 'seed': seed}, f'row {index}: seed or sampling differs')
                shift = seed % len(options)
                require(call['options'] == options[shift:] + options[:shift], f'row {index}: option rotation differs')
            outcome = row['outcome']
            require('refusal' not in outcome, f'row {index}: runtime refused')
            require(isinstance(outcome.get('choice'), str) and outcome.get('choice') in call['options'], f'row {index}: actual admitted choice missing')
            arm = row.get('arm')
            if arm in hits:
                hits[arm] += outcome.get('choice') == secret
                if arm == 'state':
                    require(secret in call['prompt'], f'row {index}: state lacks planted token')
                else:
                    require(secret not in call['prompt'], f'row {index}: token leaked into control prompt')
            # Latency and band can be null; do not invent missing runtime metadata.
        require(len(set(ids)) == len(ids), 'call IDs are not unique')
        rates = {arm: count / 8 for arm, count in hits.items()}
        report.update(hits=hits, reconstructed_rates=rates, published_isolation=isolation)
        for arm, rate in rates.items():
            require(rate == isolation[arm], f'{arm}: reconstructed rate differs from startup card')
    except (OSError, ValueError, KeyError, TypeError, IndexError, AttributeError) as error:
        errors.append(f'{type(error).__name__}: {error}')
    report['verified'] = not errors
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    parser.add_argument('models', type=Path)
    parser.add_argument('--source', type=Path, default=Path('/Users/christopherdavid/work/openagents/crates/lev/src/admission.rs'))
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    report = validate(args.capture, args.models, args.source)
    output = json.dumps(report, indent=2) + '\n'
    if args.report:
        with args.report.open('x') as stream:
            stream.write(output)
    print(output, end='')
    return 0 if report['verified'] else 1


if __name__ == '__main__':
    sys.exit(main())
