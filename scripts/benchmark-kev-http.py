"""Measure serial local Kev HTTP requests on fixed and changing public states.

This infrastructure probe retains request shape and timings, never raw state.
Run on an otherwise idle host after stopping builds and other model processes.
"""
import argparse
import http.client
import json
import math
from pathlib import Path
import time
import urllib.parse
import urllib.request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--url', default='http://127.0.0.1:18454')
    parser.add_argument('--model', required=True)
    parser.add_argument('--repeats', type=int, default=20)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.repeats < 1:
        parser.error('--repeats must be positive')
    parsed = urllib.parse.urlsplit(args.url)
    connection = http.client.HTTPSConnection if parsed.scheme == 'https' else http.client.HTTPConnection
    client = connection(parsed.hostname, parsed.port, timeout=300)
    def get(path):
        with urllib.request.urlopen(args.url + path) as response:
            return json.load(response)
    result = {'models': get('/v1/models'), 'info': get('/api/info'), 'concurrency': 1,
              'repeats': args.repeats, 'cases': []}
    sentence = 'The customer received the wrong size shoes and requests an exchange. '
    question = {'type': 'choice', 'instructions': 'Which team should handle the request?',
                'criteria': {'returns': 'Exchanges and refunds', 'shipping': 'Delivery delays', 'billing': 'Payment problems'}}
    for length, copies in [('short', 2), ('long', 48)]:
        for count in [1, 5]:
            for mode in ['repeated', 'new']:
                rows = []
                for iteration in range(args.repeats + 3):
                    state = sentence * copies
                    if mode == 'new':
                        state += f' Ticket reference: {iteration:04d}.'
                    body = json.dumps({'model': args.model, 'state': state,
                                       'questions': {f'route_{q}': question for q in range(count)}})
                    start = time.perf_counter()
                    client.request('POST', '/v1/systemone', body, {'Content-Type': 'application/json'})
                    response = client.getresponse()
                    raw = response.read()
                    elapsed = (time.perf_counter() - start) * 1000
                    if response.status != 200:
                        raise RuntimeError(f'HTTP {response.status}: {raw.decode()}')
                    answer = json.loads(raw)
                    rows.append({'warmup': iteration < 3, 'http_ms': elapsed, 'response': answer})
                measured = [r for r in rows if not r['warmup']]
                times = sorted(r['http_ms'] for r in measured)
                result['cases'].append({'state': length, 'copies': copies, 'questions': count,
                                        'mode': mode, 'p50_ms': times[math.ceil(.5 * len(times))-1],
                                        'p95_ms': times[math.ceil(.95 * len(times))-1], 'rows': rows})
                args.output.write_text(json.dumps(result, indent=2)+'\n')
                print(length, count, mode, round(result['cases'][-1]['p50_ms'], 1), flush=True)
    client.close()


if __name__ == '__main__':
    main()
