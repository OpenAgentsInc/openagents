"""Frozen cohorts with a durable launch journal and conservative spend holds."""
from __future__ import annotations

import fcntl
import hashlib
import json
import os
import re
import signal
import subprocess
import sys
import time
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation
from pathlib import Path

SCHEMA = 'openagents.tbench.cohort.v1'
RULE = 'microluna-open-request-v1'
NAME = re.compile(r'[a-z0-9][a-z0-9-]{0,100}')


class CohortError(RuntimeError):
    pass


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def money(value):
    if isinstance(value, bool) or value is None:
        raise CohortError('cost is unknown, not zero')
    try:
        number = Decimal(str(value))
    except InvalidOperation as error:
        raise CohortError('invalid cost') from error
    if not number.is_finite() or number < 0:
        raise CohortError('cost must be finite and nonnegative')
    return number


def price(usage, rule, *, before_agent=False, oracle=None):
    """One versioned rule for the live ledger and final report.

    ``oracle`` is the trial's ``oracle-host.json`` (``tbench.oracle_host``),
    when the host wrote an oracle before the agent started. Its cost is
    added under that module's declared rule, even when the trial ended
    before the agent ran.
    """
    base = _price(usage, rule, before_agent=before_agent)
    if not oracle:
        return base
    extra = oracle.get('cost') or {}
    counted = money(extra.get('counted_usd'))
    recorded = extra.get('recorded_usd')
    recorded = money(recorded) if recorded is not None else None
    out = dict(base, oracle_host={'status': oracle.get('status'), 'digest': oracle.get('digest'),
                                  'recorded_usd': None if recorded is None else str(recorded),
                                  'counted_usd': str(counted)})
    proven = base.get('recorded_usd') or base.get('lower_bound_usd') or '0'
    out['lower_bound_usd'] = str(money(proven) + (recorded or 0))
    if base['counted_usd'] is None:
        return out
    out['counted_usd'] = str(money(base['counted_usd']) + counted)
    if base.get('recorded_usd') is not None and recorded is not None:
        out['recorded_usd'] = str(money(base['recorded_usd']) + recorded)
    else:
        out['recorded_usd'] = None
    if out['kind'] in ('known-zero', 'known-zero-before-agent') and counted > 0:
        out['kind'] = 'host-oracle-before-agent' if before_agent else 'recorded'
    return out


def _price(usage, rule, *, before_agent=False):
    if rule['version'] != RULE:
        raise CohortError('unsupported cost rule')
    if before_agent:
        if usage is not None:
            raise CohortError('pre-agent zero conflicts with an inference usage record')
        return {'kind': 'known-zero-before-agent', 'recorded_usd': '0', 'counted_usd': '0'}
    if usage is None:
        return {'kind': 'unknown', 'recorded_usd': None, 'counted_usd': None}
    cost = usage['cost']
    amount = cost.get('amount_usd')
    lower = cost.get('lower_bound_usd')
    if amount is not None:
        exact = money(amount)
        if lower is not None and money(lower) > exact:
            raise CohortError('cost total is below its recorded lower bound')
        if cost.get('unknown_calls'):
            raise CohortError('complete cost conflicts with unknown calls')
        return {'kind': 'known-zero' if exact == 0 else 'recorded',
                'recorded_usd': str(exact), 'counted_usd': str(exact)}
    components = usage.get('components') or {}
    jev = (components.get('jev') or {}).get('cost_usd')
    dispatches = (components.get('delegate') or {}).get('dispatches') or []
    generation = components.get('generation') or {}
    # This rule covers one Microluna dispatch and fully priced Jev calls.
    # A second dispatch or an unknown generation call needs a different rule.
    bounded = (len(dispatches) == 1 and dispatches[0].get('agent') == 'microluna'
               and cost.get('unknown_calls') == 1
               and dispatches[0].get('status') == 'timed_out' and dispatches[0].get('charge') == 'unknown'
               and generation.get('cost_usd') == 0 and not generation.get('unpriced_calls')
               and not generation.get('failed_calls_unknown_charge')
               and not (components.get('jev') or {}).get('unknown'))
    if lower is not None and jev is not None and bounded:
        counted = max(money(lower), money(rule['luna_bound_usd']) + money(jev))
        return {'kind': 'open-request-bound', 'recorded_usd': None,
                'lower_bound_usd': str(money(lower)), 'counted_usd': str(counted)}
    return {'kind': 'unknown', 'recorded_usd': None,
            'lower_bound_usd': str(money(lower)) if lower is not None else None, 'counted_usd': None}


def validate(spec):
    if spec.get('schema') != SCHEMA or not NAME.fullmatch(spec.get('id', '')):
        raise CohortError('invalid cohort schema or id')
    if money(spec['budget_usd']) <= 0 or money(spec['reservation_usd']) <= 0:
        raise CohortError('budget and per-attempt reservation must be positive')
    if spec['cost_rule']['version'] != RULE:
        raise CohortError('unsupported cost rule')
    if money(spec['reservation_usd']) <= money(spec['cost_rule']['luna_bound_usd']):
        raise CohortError('reservation must cover at least the Luna bound plus headroom for Jev')
    for key, low, high in [('concurrency', 1, 8), ('retries', 0, 3)]:
        if type(spec[key]) is not int or not low <= spec[key] <= high:
            raise CohortError(f'invalid {key}')
    slots = spec['schedule']
    if not slots or len({r['id'] for r in slots}) != len(slots):
        raise CohortError('the schedule must have unique slots')
    for row in slots:
        if not NAME.fullmatch(row['id']):
            raise CohortError('invalid slot id')
        if 'policy_path' in row and not (isinstance(row['policy_path'], str) and row['policy_path']):
            raise CohortError('a slot policy_path must be a path')


def slot_policy(spec, slot):
    """The manifest a slot runs: its own ``policy_path``, else the spec's.
    A per-slot manifest lets one cohort interleave matched arms under one
    budget and one journal."""
    return slot.get('policy_path') or spec['policy_path']


def read_events(path: Path, *, allow_unterminated_tail=False):
    """Reads a journal and checks its hash chain.

    A driver writes each event as one line that ends in a newline. With
    ``allow_unterminated_tail``, a final line without its newline is left out,
    because a driver may be writing it, and the second value says so.
    """
    lines = path.read_text().split('\n')
    tail = lines.pop()
    if tail and not allow_unterminated_tail:
        lines.append(tail)
    events = []
    for raw in lines:
        event = json.loads(raw)
        recorded = event.pop('digest')
        if (digest(event) != recorded or event['sequence'] != len(events)
                or event['previous'] != (events[-1]['digest'] if events else None)):
            raise CohortError('cohort journal hash chain is invalid')
        event['digest'] = recorded
        events.append(event)
    return events, bool(tail) and allow_unterminated_tail


class Ledger:
    """Attempts and spend computed from journal events, without writing."""
    def __init__(self, spec: dict, events: list):
        self.spec, self.events = spec, events

    def attempts(self):
        rows = {}
        for event in self.events:
            if event['kind'] == 'reserve':
                rows[event['job']] = dict(event, state='reserved')
            elif event['kind'] == 'started':
                rows[event['job']].update(state='running', pid=event['pid'])
            elif event['kind'] == 'settle':
                rows[event['job']].update(state='settled', result=event['result'])
        return rows

    def accounting(self):
        """Counts spend against the ceiling and bounds what was spent.

        ``counted_usd`` is what the cost rule counts for settled attempts.
        ``lower_bound_usd`` adds only amounts the usage records prove: exact
        totals and the recorded part of an unfinished request.
        ``upper_bound_usd`` is the counted amount plus every hold. It is the
        most this cohort charges against its ceiling under the rule, not a
        limit on a provider's bill.
        """
        charged, held, proven = Decimal(0), Decimal(0), Decimal(0)
        unknown = []
        open_request = []
        breached = []
        for row in self.attempts().values():
            cost = row.get('result', {}).get('cost', {})
            counted = cost.get('counted_usd')
            known = cost.get('recorded_usd')
            if known is None:
                known = cost.get('lower_bound_usd')
            if known is not None:
                proven += money(known)
            if cost.get('kind') == 'open-request-bound':
                open_request.append(row['job'])
            reserve = money(row['reservation_usd'])
            if row['state'] != 'settled' or counted is None:
                lower = cost.get('lower_bound_usd')
                held += max(reserve, money(lower) if lower is not None else Decimal(0))
                if row['state'] == 'settled':
                    unknown.append(row['job'])
            else:
                charged += money(counted)
                if money(counted) > reserve:
                    breached.append(row['job'])
        return {'counted_usd': str(charged), 'held_usd': str(held), 'unknown_jobs': unknown,
                'open_request_jobs': open_request,
                'lower_bound_usd': str(proven), 'upper_bound_usd': str(charged + held),
                'reservation_breaches': breached,
                'available_usd': str(money(self.spec['budget_usd']) - charged - held)}


def status(directory: Path):
    """Summarizes a cohort's spend from its journal without locking or writing.

    It is safe to run while a driver owns the cohort.
    """
    path = directory / 'ledger.jsonl'
    if not path.exists():
        raise CohortError('no cohort journal in this directory')
    events, unterminated = read_events(path, allow_unterminated_tail=True)
    if not events or events[0]['kind'] != 'pin':
        raise CohortError('the cohort journal does not start with its pinned spec')
    spec = events[0]['spec']
    view = Ledger(spec, events)
    rows = list(view.attempts().values())
    spend = view.accounting()
    return {'schema': 'openagents.tbench.cohort-status.v1', 'id': spec['id'],
            'ledger_head': events[-1]['digest'], 'events': len(events),
            'unterminated_tail': unterminated,
            'budget_usd': spec['budget_usd'], 'spent_usd': spend['counted_usd'],
            'held_usd': spend['held_usd'],
            'lower_bound_usd': spend['lower_bound_usd'],
            'upper_bound_usd': spend['upper_bound_usd'],
            'unknown_cost_count': len(spend['unknown_jobs']) + len(spend['open_request_jobs']),
            'remaining_usd': spend['available_usd'],
            'attempts': len(rows), 'running': sum(r['state'] != 'settled' for r in rows),
            'graded': sum(r.get('result', {}).get('kind') == 'graded' for r in rows),
            'planned': len(spec['schedule']),
            'deviations': sum(e['kind'] == 'deviation' for e in events)}


class Journal(Ledger):
    """An exclusive, fsynced append-only journal. Ambiguous launches keep holds."""
    def __init__(self, directory: Path, spec: dict, identity: dict):
        validate(spec)
        directory.mkdir(parents=True, exist_ok=True)
        self.directory, self.spec, self.identity = directory, spec, identity
        self.lock = (directory / 'lock').open('a+')
        try:
            fcntl.flock(self.lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            self.lock.close()
            raise CohortError('another driver owns this cohort') from error
        self.path = directory / 'ledger.jsonl'
        self.events = []
        try:
            if self.path.exists():
                self.events = read_events(self.path)[0]
            if not self.events:
                self.append('pin', spec=spec, identity=identity)
            elif self.events[0]['spec'] != spec or self.events[0]['identity'] != identity:
                self.append('deviation', reason='spec or source/artifact identity changed',
                            attempted_spec_digest=digest(spec), attempted_identity=identity)
                raise CohortError('cohort is frozen; source or spec changed, and the deviation was retained')
            if identity.get('source_dirty'):
                self.append('deviation', reason='uncommitted source; launch refused')
                raise CohortError('cohort execution requires a clean, committed checkout')
            self.append('epoch', pid=os.getpid(), identity=identity)
        except BaseException:
            self.close()
            raise

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()

    def close(self):
        self.lock.close()

    def append(self, kind, **data):
        event = {'kind': kind, 'at': datetime.now(timezone.utc).isoformat(),
                 'sequence': len(self.events), 'previous': self.events[-1]['digest'] if self.events else None, **data}
        event['digest'] = digest(event)
        with self.path.open('a') as stream:
            stream.write(json.dumps(event, sort_keys=True) + '\n')
            stream.flush()
            os.fsync(stream.fileno())
        descriptor = os.open(self.directory, os.O_RDONLY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        self.events.append(event)
        return event

    def reserve(self, slot):
        attempts = [r for r in self.attempts().values() if r['slot'] == slot]
        if attempts:
            last = attempts[-1]
            if last['state'] != 'settled':
                return last
            if last['result']['kind'] != 'infrastructure' or len(attempts) > self.spec['retries']:
                return None
        budget = self.accounting()
        active = sum(r['state'] != 'settled' for r in self.attempts().values())
        if (budget['unknown_jobs'] or budget['reservation_breaches'] or active >= self.spec['concurrency']
                or Decimal(budget['available_usd']) < money(self.spec['reservation_usd'])):
            return None
        if slot not in {s['id'] for s in self.spec['schedule']}:
            raise CohortError('slot is outside the frozen schedule')
        number = len(attempts) + 1
        job = f"cohort-{self.spec['id']}--{slot}--try-{number}"
        self.append('reserve', slot=slot, attempt=number, job=job, reservation_usd=self.spec['reservation_usd'])
        return self.attempts()[job]

    def started(self, job, pid):
        row = self.attempts()[job]
        if row['state'] != 'reserved':
            raise CohortError('a reserved attempt can be started only once')
        self.append('started', job=job, pid=pid)

    def settle(self, job, result):
        row = self.attempts()[job]
        if row['state'] == 'settled':
            if row['result'] != result:
                raise CohortError('a settled outcome cannot be replaced')
            return
        self.append('settle', job=job, result=result)

    def report(self):
        rows = list(self.attempts().values())
        completed = sum(r.get('result', {}).get('kind') == 'graded' for r in rows)
        report = {'schema': SCHEMA, 'spec': self.spec, 'identity': self.identity,
                  'ledger_head': self.events[-1]['digest'], 'accounting': self.accounting(),
                  'graded': completed, 'planned': len(self.spec['schedule']),
                  'passes': sum(r.get('result', {}).get('reward') == 1 for r in rows),
                  'attempts': rows,
                  'deviations': [e for e in self.events if e['kind'] == 'deviation'],
                  'complete': completed == len(self.spec['schedule']),
                  'accounting_complete': not self.accounting()['unknown_jobs'] and all(r['state'] == 'settled' for r in rows)}
        target = self.directory / 'report.json'
        temporary = target.with_suffix('.tmp')
        temporary.write_text(json.dumps(report, indent=2) + '\n')
        temporary.replace(target)
        return report


def inspect(job: Path, rule):
    results = list(job.glob('*/result.json'))
    if not results:
        refusals = list((job / 'tbench/refusals').glob('*.json'))
        if len(refusals) == 1:
            record = json.loads(refusals[0].read_text())
            if record.get('schema') == 'openagents.tbench.refusal.v1' and record.get('terminal_status') == 'setup_failure':
                return {'kind': 'infrastructure', 'reward': None, 'cost': price(None, rule, before_agent=True),
                        'refusal_sha256': hashlib.sha256(refusals[0].read_bytes()).hexdigest()}
        return None
    if len(results) != 1:
        raise CohortError('a cohort job must contain exactly one trial')
    result = json.loads(results[0].read_text())
    if not result.get('finished_at'):
        return None
    usage_path = results[0].parent / 'agent/episode/evaluation/usage.json'
    usage = json.loads(usage_path.read_text()) if usage_path.exists() else None
    oracle_path = results[0].parent / 'agent/oracle-host.json'
    oracle = json.loads(oracle_path.read_text()) if oracle_path.exists() else None
    before = ('agent_execution' in result and result['agent_execution'] is None
              and result.get('agent_result') is None and bool(result.get('exception_info')))
    reward = ((result.get('verifier_result') or {}).get('rewards') or {}).get('reward')
    graded = type(reward) in (int, float) and reward in (0, 1)
    return {'kind': 'graded' if graded else ('infrastructure' if before else 'incomplete'),
            'reward': reward if graded else None,
            'cost': price(usage, rule, before_agent=before and not graded, oracle=oracle),
            'result_sha256': hashlib.sha256(results[0].read_bytes()).hexdigest(),
            'usage_sha256': hashlib.sha256(usage_path.read_bytes()).hexdigest() if usage_path.exists() else None}


def slot_request(slot):
    from .cli import _load
    request = _load(slot['profile'], slot['agent'])
    if request is not None and request.checkout is None:
        raise CohortError('the pinned task checkout is missing; run tbench tasks checkout first')
    if (request is None or request.profile.n_attempts != 1
            or request.profile.retry.get('max_retries', 0) != 0 or request.profile.install_only
            or request.profile.environment.get('import_path') != 'tbench.warm_docker:WarmDockerEnvironment'
            or request.agent.harbor_import_path not in ('tbench.coder_one:CoderOne', 'tbench.coder_one:CoderOneTunable')):
        raise CohortError('a cohort slot requires a single-attempt Coder One profile with warm Docker and no hidden retries')
    task = request.panel.select([Path(slot['task_path']).name])[0]
    if (request.checkout / task.path).resolve() != Path(slot['task_path']).expanduser().resolve():
        raise CohortError('slot task_path differs from the task the run command would execute')
    request.tasks = [task]
    return request


def identity(checkout, spec):
    from harbor.models.task.task import Task
    from .candidate_capture import check_policy
    changed = subprocess.check_output(['git', '-C', str(checkout), 'status', '--porcelain'], text=True)
    artifact = Path(spec['artifact_path']).expanduser()
    paths = [spec['policy_path']] + [r['policy_path'] for r in spec['schedule'] if r.get('policy_path')]
    if not artifact.is_absolute() or not all(Path(p).expanduser().is_absolute() for p in paths):
        raise CohortError('artifact_path and policy_path must be absolute')
    artifact_hash = hashlib.sha256(artifact.read_bytes()).hexdigest()
    if artifact_hash != spec['artifact_sha256']:
        raise CohortError('artifact differs from its pinned digest')
    digests = {}
    for path in dict.fromkeys(paths):
        policy = Path(path).expanduser()
        policy_data = json.loads(policy.read_text())
        check_policy(policy_data)
        executor = policy_data['policy']['executor']
        if executor['agent'] != 'microluna' or money(executor['microluna']['spend_usd']) != money(spec['cost_rule']['luna_bound_usd']):
            raise CohortError('cost rule must match the pinned Microluna policy budget')
        digests[path] = hashlib.sha256(policy.read_bytes()).hexdigest()
    for slot in spec['schedule']:
        slot_request(slot)
    pinned = {'source_dirty': hashlib.sha256(changed.encode()).hexdigest() if changed.strip() else None,
              'source_commit': subprocess.check_output(['git', '-C', str(checkout), 'rev-parse', 'HEAD'], text=True).strip(),
              'artifact_sha256': artifact_hash, 'policy_sha256': digests[spec['policy_path']],
              'tasks': {r['id']: Task(Path(r['task_path']).expanduser()).checksum for r in spec['schedule']}}
    # Only a cohort with per-slot manifests records them, so an existing
    # journal's pinned identity is unchanged.
    if len(paths) > 1:
        pinned['slot_policy_sha256'] = {r['id']: digests[slot_policy(spec, r)] for r in spec['schedule']}
    return pinned


def launch(journal, slot, job, checkout):
    command = [sys.executable, '-m', 'tbench', 'run', '--profile', slot['profile'],
        '--agent', slot['agent'], '--task', Path(slot['task_path']).name, '--job-name', job,
        '--agent-kwarg', 'artifact_path=' + journal.spec['artifact_path'],
        '--agent-kwarg', 'artifact_sha256=' + journal.spec['artifact_sha256'],
        '--agent-kwarg', 'policy=' + slot_policy(journal.spec, slot),
        '--agent-kwarg', 'candidate_capture=true']
    with (journal.directory / (job + '.log')).open('ab') as log:
        return subprocess.Popen(command, cwd=checkout / 'bench/terminal-bench',
            stdout=log, stderr=subprocess.STDOUT, start_new_session=True)


def run(journal, jobs_dir, checkout):
    """Run a fixed queue; every process starts after its durable reservation."""
    from .suite import Host, running_pid
    from .host import docker_info
    from .candidate_capture import preflight
    host = Host.docker(checkout)
    info = docker_info()
    cpu_capacity = info['NCPU']
    memory_capacity = info['MemTotal'] / 1024**2
    for slot in journal.spec['schedule']:
        coverage = preflight(Path(slot['task_path']))
        if not coverage['supported']:
            raise CohortError('candidate coverage preflight failed: ' + '; '.join(coverage['reasons']))
    resources = {r['id']: slot_request(r).tasks[0].peak_resources for r in journal.spec['schedule']}
    # Source and artifact identity is checked again before every launch.
    children = {}
    try:
        while True:
            rows = journal.attempts()
            for job, row in rows.items():
                if row['state'] == 'settled':
                    continue
                if job in children and children[job].poll() is None:
                    continue
                if job not in children and running_pid(job) is not None:
                    continue
                outcome = inspect(jobs_dir / job, journal.spec['cost_rule'])
                if outcome:
                    journal.settle(job, outcome)
                elif job not in children and running_pid(job) is None:
                    journal.settle(job, {'kind': 'interrupted', 'reward': None,
                        'cost': price(None, journal.spec['cost_rule']),
                        'reason': 'launch or completion is ambiguous; reservation retained'})
                elif job in children and children[job].poll() is not None:
                    journal.settle(job, {'kind': 'incomplete', 'reward': None,
                        'cost': price(None, journal.spec['cost_rule']), 'exit': children[job].returncode})
            launched = False
            for slot in journal.spec['schedule']:
                previous = [r for r in journal.attempts().values() if r['slot'] == slot['id']]
                if previous and (previous[-1]['state'] != 'settled' or previous[-1]['result']['kind'] != 'infrastructure'
                                 or len(previous) > journal.spec['retries']):
                    continue
                active = [r for r in journal.attempts().values() if r['state'] != 'settled']
                needed = resources[slot['id']]
                if needed.cpus > cpu_capacity or needed.memory_mb > memory_capacity or needed.gpus:
                    raise CohortError('a task exceeds this Docker host; no launch')
                if (sum(resources[r['slot']].cpus for r in active) + needed.cpus > cpu_capacity or
                    sum(resources[r['slot']].memory_mb for r in active) + needed.memory_mb > memory_capacity):
                    break
                if host.free_disk_gb() < 20:
                    raise CohortError('Docker volume has less than 20 GiB free; launch refused')
                try:
                    current_identity = identity(checkout, journal.spec)
                except (OSError, ValueError, KeyError, CohortError) as error:
                    journal.append('deviation', reason='source or artifact could not be verified before launch')
                    raise CohortError('identity could not be verified before launch') from error
                if current_identity != journal.identity:
                    journal.append('deviation', reason='source or artifact changed before launch')
                    raise CohortError('identity changed before launch')
                reserved = journal.reserve(slot['id'])
                if reserved is None:
                    break
                job = reserved['job']
                if (jobs_dir / job).exists():
                    raise CohortError('reserved job directory already exists; inspect before resuming')
                child = launch(journal, slot, job, checkout)
                children[job] = child
                journal.started(job, child.pid)
                launched = True
            report = journal.report()
            active = any(r['state'] != 'settled' for r in journal.attempts().values())
            if not active and not launched:
                return report
            time.sleep(1)
    except BaseException:
        journal.append('interrupted', reason='driver stopped; all unpriced reservations remain held')
        for child in children.values():
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGTERM)
        journal.report()
        raise
