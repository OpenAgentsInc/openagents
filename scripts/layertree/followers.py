#!/usr/bin/env python3
"""Launch research followers and check retained P1 evidence. No wallet access."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import sys
import subprocess
import time

WEEK = 7 * 24 * 60 * 60
MAX_GAP = 120


def require(condition, message):
    if not condition:
        raise ValueError(message)


def rows(path):
    return [json.loads(line) for line in Path(path).read_text().splitlines() if line]


def write_json(path, data):
    path = Path(path)
    temp = path.with_suffix('.new')
    with temp.open('w') as stream:
        json.dump(data, stream, indent=2)
        stream.write('\n')
        stream.flush()
        os.fsync(stream.fileno())
    temp.replace(path)


def resolve(base, path):
    return (base / path).resolve()


def run(args):
    configs = [Path(path).resolve() for path in args.configs]
    require(len(set(configs)) == 3, 'Choose three distinct configurations.')
    require(args.until > int(time.time()), 'Choose a future end time.')
    states, identities = set(), set()
    for path in configs:
        cfg = json.loads(path.read_text())
        state = resolve(path.parent, cfg['state'])
        states.add(state)
        identities.add((cfg['network'], cfg['member']))
        require(cfg.get('webhook_env') in os.environ, 'Set each configured alarm webhook.')
    require(len(states) == len(identities) == 3, 'Choose distinct state directories and members.')
    binary = Path(args.binary).resolve()
    out = Path(args.output).resolve()
    out.mkdir(parents=True, exist_ok=True)
    manifest_path = out / 'processes.json'
    require(not manifest_path.exists(), 'Use a new launch directory; keep previous evidence.')
    manifest = {'schema': 'layertree.followers.launch.v1', 'started_at': int(time.time()),
                'until': args.until, 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                'processes': []}
    children, streams = [], []
    interrupted = False

    def stop(_signum, _frame):
        nonlocal interrupted
        interrupted = True
        for child in children:
            if child.poll() is None:
                child.terminate()

    previous = {sig: signal.signal(sig, stop) for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        for index, path in enumerate(configs):
            stdout = (out / f'{index}.jsonl').open('xb')
            stderr = (out / f'{index}.log').open('xb')
            streams.extend([stdout, stderr])
            child = subprocess.Popen([str(binary), '--config', str(path), '--until', str(args.until)],
                                     stdout=stdout, stderr=stderr)
            children.append(child)
            manifest['processes'].append({'config': str(path), 'pid': child.pid,
                                          'config_sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
            write_json(manifest_path, manifest)
        reported = set()
        while any(child.poll() is None for child in children):
            for index, child in enumerate(children):
                code = child.poll()
                if code is not None and index not in reported:
                    reported.add(index)
                    manifest['processes'][index].update(exit_code=code, finished_at=int(time.time()))
                    write_json(manifest_path, manifest)
                    print(f'Follower {index} exited with {code}; inspect {index}.log.', file=sys.stderr, flush=True)
            time.sleep(1)
        codes = [child.returncode for child in children]
        manifest.update(finished_at=int(time.time()), exit_codes=codes, interrupted=interrupted)
        write_json(manifest_path, manifest)
        require(not interrupted and codes == [0, 0, 0],
                f'Followers exited with {codes}; inspect their logs and retry queued notifications.')
    finally:
        for child in children:
            if child.poll() is None:
                child.terminate()
        for child in children:
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
        for stream in streams:
            stream.close()
        for sig, handler in previous.items():
            signal.signal(sig, handler)


def read_group(base, paths, inputs):
    require(len(paths) == len(set(paths)) == 3, 'Each evidence group needs three distinct state directories.')
    group, members = [], set()
    for path in paths:
        log = resolve(base, path) / 'observations.jsonl'
        data = rows(log)
        inputs[str(log)] = hashlib.sha256(log.read_bytes()).hexdigest()
        checks = [row['status'] for row in data if row['kind'] == 'checked']
        require(bool(checks), f'No completed checks in {log}.')
        identity = {(row['network'], row['tree_id'], row['member']) for row in checks}
        require(len(identity) == 1 and next(iter(identity))[0] == 'signet', 'Evidence must follow one signet tree per process.')
        members.add(checks[0]['member'])
        group.append((data, checks))
    require(len(members) == 3, 'Evidence must come from three different members.')
    require(len({checks[0]['tree_id'] for _, checks in group}) == 1, 'Followers must watch the same tree.')
    return group


def check(evidence):
    """Consistency gate, not independent verification of signet consensus or hardware."""
    path = Path(evidence).resolve()
    base = path.parent
    plan = json.loads(path.read_text())
    require(plan['schema'] == 'layertree.followers.evidence.v1', 'Unknown evidence schema.')
    inputs = {}
    operator_path = resolve(base, plan['operator'])
    operator = rows(operator_path)
    inputs[str(operator_path)] = hashlib.sha256(operator_path.read_bytes()).hexdigest()
    require(operator and operator[0]['kind'] == 'tree_open' and
            all(row['network'] == 'signet' for row in operator), 'Choose the public signet operator log.')
    honest = read_group(base, plan['honest'], inputs)
    start = max(checks[0]['checked_at'] for _, checks in honest)
    end = min(checks[-1]['checked_at'] for _, checks in honest)
    require(end - start >= WEEK, 'Three followers have not completed a common week.')
    transitions = [row for row in operator if row['kind'] == 'transition' and row.get('height') is not None]
    require(bool(transitions), 'No confirmed operator transitions.')
    pids = set()
    for data, checks in honest:
        require(checks[0]['tree_id'] == operator[0]['tree_id'], 'Operator and follower tree IDs differ.')
        times = [row['checked_at'] for row in checks]
        require(all(0 <= b - a <= MAX_GAP for a, b in zip(times, times[1:])), 'Follower check gap exceeds two minutes.')
        require(all(row['stopped'] is None and not row['exited'] and row['unroll_held'] for row in checks),
                'A public-tree follower stopped, exited, or lacked its escape package.')
        require(all(row['scanned'] == row['observed_tip'] for row in checks), 'A public-tree follower fell behind.')
        require(checks[-1]['observed_tip'] > checks[0]['observed_tip'], 'The chain did not advance.')
        require(not any(row['kind'] == 'light_client' and row['result'] != 'ok' for row in data),
                'The honest public tree raised an alarm.')
        followed = {row['txid'] for row in data if row['kind'] == 'light_client' and row['result'] == 'ok'}
        expected = [row for row in transitions if row['height'] <= checks[-1]['scanned']]
        require(expected and all(row['txid'] in followed for row in expected), 'A confirmed transition was not followed.')
        require(checks[-1]['outpoint'] == expected[-1]['txid'] + ':0', 'Final follower output differs from the operator log.')
        pids.add(checks[0]['pid'])
    require(len(pids) == 3, 'The three initial follower process IDs must differ.')
    receipt_path = resolve(base, plan['notifications'])
    receipts = rows(receipt_path)
    inputs[str(receipt_path)] = hashlib.sha256(receipt_path.read_bytes()).hexdigest()
    delivered = {row['id'] for row in receipts}
    alarms = {}
    for scenario, result in [('theft', 'OutputMismatch'), ('withheld', 'RecordMissing'), ('mismatch', 'RecordNotAnchored')]:
        group = read_group(base, plan['attacks'][scenario], inputs)
        require(group[0][1][0]['tree_id'] != operator[0]['tree_id'], 'Stage attacks on a separate test tree.')
        ids, spend_sets = [], []
        for data, _ in group:
            acks = {row['id'] for row in data if row['kind'] == 'webhook_ack'}
            matches = [row for row in data if row['kind'] == 'light_client' and row['result'] == result]
            require(bool(matches), f'No {scenario} alarm in every follower.')
            spend_sets.append({(row['height'], row['txid']) for row in matches})
            for row in matches:
                require(row['network'] == 'signet' and row['id'] in acks and row['id'] in delivered, f'An undelivered {scenario} alarm remains.')
                if scenario == 'theft':
                    require(row['height'] <= row['observed_tip'] <= row['height'] + 1,
                            'Theft was detected more than one block after mining.')
                ids.append(row['id'])
        require(bool(set.intersection(*spend_sets)), f'Followers did not alarm on the same {scenario} spend.')
        alarms[scenario] = ids
    return {'schema': 'layertree.followers.acceptance.v1', 'common_seconds': end - start,
            'start': start, 'end': end, 'alarms': alarms, 'sha256': inputs}


def merge(args):
    operator = rows(args.operator)
    require(operator and operator[0]['kind'] == 'tree_open', 'Choose a full operator event log.')
    network, tree = operator[0]['network'], operator[0]['tree_id']
    observations = {}
    for state in args.states:
        for row in rows(Path(state) / 'observations.jsonl'):
            if row['kind'] != 'light_client':
                continue
            require((row['network'], row['tree_id']) == (network, tree), 'Observation belongs to another tree.')
            observations[row['id']] = row
    pending = sorted(observations.values(), key=lambda row: (row['height'], row['observed_at'], row['id']))
    result = []
    for index, event in enumerate(operator):
        result.append(event.copy())
        # Preserve operator order, including transitions in the same block.
        following = operator[index + 1: index + 2]
        boundary = following[0].get('height') if following else None
        if index and boundary is not None and boundary > (event.get('height') or 0):
            while pending and pending[0]['height'] < boundary:
                row = pending.pop(0)
                result.append({'network': network, 'height': row['height'], 'kind': 'light_client',
                               'member': row['member'], 'result': row['result']})
    for row in pending:
        result.append({'network': network, 'height': row['height'], 'kind': 'light_client',
                       'member': row['member'], 'result': row['result']})
    target = Path(args.output)
    with target.open('x') as stream:
        for seq, row in enumerate(result):
            row['seq'] = seq
            stream.write(json.dumps(row, separators=(',', ':')) + '\n')
    sources = [Path(args.operator)] + [Path(s) / 'observations.jsonl' for s in args.states]
    write_json(target.with_suffix('.sources.json'),
               {str(p.resolve()): hashlib.sha256(p.read_bytes()).hexdigest() for p in sources})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    launch = commands.add_parser('run', help='Run three child processes until an explicit deadline.')
    launch.add_argument('--binary', required=True)
    launch.add_argument('--until', type=int, required=True)
    launch.add_argument('--output', required=True)
    launch.add_argument('configs', nargs=3)
    acceptance = commands.add_parser('check', help='Require a common signet week and delivered staged alarms.')
    acceptance.add_argument('--evidence', required=True)
    acceptance.add_argument('--output', required=True)
    combine = commands.add_parser('merge', help='Add follower alarms to an operator log for Verse.')
    combine.add_argument('--operator', required=True)
    combine.add_argument('--states', nargs=3, required=True)
    combine.add_argument('--output', required=True)
    args = parser.parse_args()
    try:
        if args.command == 'run':
            run(args)
        elif args.command == 'merge':
            merge(args)
        else:
            write_json(args.output, check(args.evidence))
    except (ValueError, KeyError, OSError, json.JSONDecodeError) as error:
        parser.exit(1, f'followers: {error}\n')


if __name__ == '__main__':
    main()
