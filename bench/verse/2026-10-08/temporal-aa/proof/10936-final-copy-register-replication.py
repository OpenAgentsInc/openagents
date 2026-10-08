#!/usr/bin/env python3
"""Register the frozen three-trial protocol before any final capture starts."""
import argparse
import json
from pathlib import Path
import time

SOURCE = 'fb9a5fd280db673926cfd0d649f924da3091a24b'
SCRATCH = Path(__file__).resolve().parent
STEM = '10936-final-copy'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', required=True)
    args = parser.parse_args()
    if args.source != SOURCE:
        raise ValueError('Use the exact frozen final runtime source')
    destination = SCRATCH / (STEM + '-replication-plan.json')
    if destination.exists():
        raise ValueError('Retain the existing registration; do not overwrite it')
    labels = ['high-dev', 'high-release', 'high-paired', 'medium-paired',
              'pan-paired', 'orbit-paired', 'high-replicate-2', 'high-replicate-3']
    for label in labels:
        prefix = SCRATCH / (STEM + '-' + label)
        for suffix in ['', '.log', '-manifest.json', '-check.json', '-gpu.json', '-quiet.json']:
            if Path(str(prefix) + suffix).exists():
                raise ValueError('Register before any final timing artifact exists: ' + str(prefix))
    value = json.loads((SCRATCH / (STEM + '-replication-plan.template.json')).read_text())
    if not (value['runtime_source'] == SOURCE and value['registered_unix'] is None
            and value['total_trials'] == 3 and value['additional_trials'] == 2
            and value['known_first_results'] is False):
        raise ValueError('The unregistered three-trial template changed')
    value['registered_unix'] = time.time()
    destination.write_text(json.dumps(value, indent=2) + '\n')
    print(destination)


if __name__ == '__main__':
    main()
