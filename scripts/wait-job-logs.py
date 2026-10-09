#!/usr/bin/env python3
"""Stream NAME.log while waiting for NAME.exit in a job directory."""
import argparse
import pathlib
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=pathlib.Path)
    parser.add_argument('names', nargs='+')
    parser.add_argument('--timeout', type=float, default=100)
    parser.add_argument('--interval', type=float, default=5)
    args = parser.parse_args()
    if args.timeout <= 0 or args.interval <= 0:
        parser.error('timeout and interval must be positive')
    if any(pathlib.Path(name).name != name for name in args.names):
        parser.error('job names must be basenames')
    started = time.monotonic()
    offsets = dict.fromkeys(args.names, 0)
    while True:
        statuses = []
        codes = []
        for name in args.names:
            log = args.directory / (name + '.log')
            try:
                with log.open('rb') as stream:
                    if log.stat().st_size < offsets[name]:
                        offsets[name] = 0
                    stream.seek(offsets[name])
                    # Bound each poll without loading a large log in memory.
                    data = stream.read(65536)
                    offsets[name] = stream.tell()
                if data:
                    print(f'[{name}] {data.decode("utf-8", errors="replace")}', end='', flush=True)
            except FileNotFoundError:
                pass
            try:
                code = int((args.directory / (name + '.exit')).read_text().strip())
                codes.append(code)
                statuses.append(f'{name} finished: exit {code}')
            except FileNotFoundError:
                statuses.append(f'{name} running')
            except ValueError:
                print(f'Invalid exit status for {name}', flush=True)
                return 1
        elapsed = time.monotonic() - started
        print(f'\n[{elapsed:05.1f}s] ' + '; '.join(statuses), flush=True)
        if len(codes) == len(args.names):
            # Drain any remaining log chunks before returning.
            if any((args.directory / (n + '.log')).exists() and
                   (args.directory / (n + '.log')).stat().st_size > offsets[n]
                   for n in args.names):
                continue
            return int(any(codes))
        if elapsed >= args.timeout:
            print('Wait timed out; background jobs have not been canceled.', flush=True)
            return 124
        time.sleep(min(args.interval, args.timeout - elapsed))


if __name__ == '__main__':
    raise SystemExit(main())
