"""Writes tasks.json for the 2026-09-25 Luna-sized family pre-registration.

Reads only retained repository files: Fable 5.1's public TB4 trials, the TB4
task list and catalog, the truthful-checks label set, the stall-detection
split, and the task anatomy. The shape and check judgments below come from
each task's public instruction. It runs no trial and calls no model.

Usage, from the repository root:

    python3 bench/terminal-bench/experiments/2026-09-25-luna-sized-family/select.py
"""
import hashlib
import json
import statistics as st
import sys
from collections import defaultdict
from datetime import datetime
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = sys.argv[1] if len(sys.argv) > 1 else str(HERE.parents[3])
OUT = sys.argv[2] if len(sys.argv) > 2 else str(HERE / 'tasks.json')

replays = json.load(open(ROOT + '/bench/terminal-bench/reference/fable-5.1-replays.json'))
tasks = json.load(open(ROOT + '/bench/terminal-bench/reference/tb4-leaderboard.json'))['tasks']
catalog = {t['id']: t for t in json.load(open(ROOT + '/bench/terminal-bench/profiles/tasks.json'))['catalogs']['tb4']['tasks']}


def dur(t):
    a = datetime.fromisoformat(t['started_at'])
    b = datetime.fromisoformat(t['finished_at'])
    return (b - a).total_seconds()


by = defaultdict(lambda: defaultdict(list))
for t in replays['trials']:
    by[t['task']][t['effort']].append(t)

# Exclusion rules. Each set is written from a named record; see protocol.md.
DEV_TEST = {
    'embedding-drift-monitor': 'Microluna dev set (v9 to v17)',
    'sound-change-cascade': 'Microluna dev set (v9 to v17)',
    'interleaved-vigenere': 'Microluna dev set (v9 to v17)',
    'fin-saccr-rwa': 'Microluna held-out test set (microluna-v15, 2026-09-25)',
    'gsea-proteomics': 'Microluna held-out test set (microluna-v15, 2026-09-25)',
    'shadow-relay': 'Microluna held-out test set (microluna-v15, 2026-09-25)',
    'coq-block-bound': 'Microluna held-out test set (microluna-v15, 2026-09-25)',
}
CAP_GAPS = {'sound-change-cascade', 'interleaved-vigenere', 'session-window-debug', 'shadow-relay',
            'coq-block-bound', 'fin-saccr-rwa', 'gsea-proteomics'}
# GPT-6 Luna trials in any harness: the Luna baseline (#9583), the truthful-checks label set's
# Luna rows, and Microluna jobs on the benchmark host.
LUNA_BASELINE = {'legacy-utility-triage', 'mvcc-lsm-compaction', 'heat-pump-warranty', 'ks-solver-cpp',
                 'wal-recovery-ordering', 'cad-model', 'nextjs-performance', 'embedding-drift-monitor',
                 'fin-saccr-rwa', 'sound-change-cascade', 'wdm-design', 'shadow-relay', 'uefi-bootkit',
                 'coq-block-bound'}
LUNA_LABEL_ROWS = {'cad-model', 'cargo-flight-dispatch', 'embedding-drift-monitor', 'heat-pump-warranty',
                   'interleaved-vigenere', 'ks-solver-cpp', 'mvcc-lsm-compaction', 'nextjs-performance',
                   'risk-scorer-replay', 'wal-recovery-ordering'}
MICROLUNA_HOST = {'embedding-drift-monitor', 'sound-change-cascade', 'interleaved-vigenere', 'session-window-debug',
                  'fin-saccr-rwa', 'gsea-proteomics', 'shadow-relay', 'coq-block-bound', 'html-js-filter'}
# #9584: the prospective Microluna v13 cohort, and the truthful-checks calibration half
# (Luna reviews are running on its task groups).
SIGNAL_COHORT = {'distributed-dedup', 'formal-crypto', 'freecad-impeller', 'freecad-spring-clip',
                 'math-eval-grader', 'pretrain-shard-corruption', 'shadow-relay', 'vpp-loss-divergence'}
rows = [json.loads(line) for line in open(ROOT + '/crates/coder-one/fixtures/truth/rows.jsonl')]
SIGNAL_CALIBRATION = {r['task'].split('/')[-1] for r in rows if r['split'] == 'calibration'}
STALL_SPLIT = set(json.load(open(ROOT + '/bench/terminal-bench/experiments/2026-09-25-stall-detection/split.json'))['calibration']) | \
    set(json.load(open(ROOT + '/bench/terminal-bench/experiments/2026-09-25-stall-detection/split.json'))['evaluation'])
anatomy = json.load(open(ROOT + '/docs/terminal-bench/2026-09-24-task-anatomy.json'))
ANATOMY = set(anatomy['feasibility_order']['A']) | set(anatomy['feasibility_order']['B']) | set(anatomy['pending'])
# Targeted Coder One experiments on a mechanism microluna-v15 turns on (host turn-backs of a
# finish, the Coder One controller and its Jev briefing).
TARGETED = {
    'production-planning': 'target of the persistence experiments (persist-v8, persist-v10) and escalation (#9570, #9571); v15 turns persistence on',
    'batched-eval-parity': 'target of the matched Opus controller test (2026-09-23), which tested the Coder One controller and briefing v15 runs under',
}
GPU = {'fp8-rmsnorm-gemm', 'jax-speedrun-gpu', 'math-eval-grader'}
WALL = {'ctr-optimization': "the task's own simulated campaign runs 48 hours of 360 seconds, about 4.8 hours, past v15's 25-minute wall bound"}


def group(task):
    return task.split('-')[0]


# Judgments from each task's public instruction only (the first user message of a public
# Fable trajectory). shape: 2 repair of shipped source against stated symptoms or a normative
# local spec; 1 build or engineering to a clear specification; 0 search, inference, perception,
# proof, open optimization, or a judgment that needs outside domain knowledge.
# check: 2 the workspace ships an exact reference, a reproducer, or runnable tests or examples;
# 1 the stated criteria can be checked with a harness the agent builds from local tools;
# 0 no local ground truth.
J = {
    'atrx-vep-crispr': (0, 1, 'sequence annotation that needs domain conventions', 'a local annotation resource'),
    'batched-eval-parity': (2, 2, 'repair of a batched evaluator', 'single-example semantics are a local reference'),
    'biped-contact-dynamics': (1, 1, 'build a trajectory generator to a schema', 'a starter script shows the schema'),
    'bun-sourcemap-leak': (2, 1, 'repair of a release pipeline', 'a policy file states what may ship'),
    'cad-model': (0, 0, 'geometry read from an image', 'no local reference'),
    'cargo-flight-dispatch': (2, 1, 'repair of a planner against reported symptoms', 'data files state the operation'),
    'coq-block-bound': (0, 2, 'a formal proof', 'the build command checks the proof'),
    'ctr-optimization': (0, 1, 'tuning a live campaign', 'the API reports metrics'),
    'cumulative-layout-shift': (2, 1, 'repair of a site so pages measure zero layout shift', 'the shift is measurable in a local browser'),
    'data-anonymization': (1, 1, 'build a CLI to a policy file', 'the policy file states the transforms'),
    'distributed-dedup': (1, 1, 'implement a trait to a stated specification', 'a stated similarity rule'),
    'embedding-drift-monitor': (2, 1, 'repair of stated defects in shipped modules', 'data scenarios ship with the task'),
    'fin-saccr-rwa': (0, 0, 'a regulatory calculation that needs outside rules', 'no local reference'),
    'foodstuff-beta-activity': (0, 0, 'a lab calculation from spreadsheets and a PDF', 'no local reference'),
    'formal-crypto': (0, 1, 'recover a plaintext by cryptanalysis', 'a plaintext and ciphertext sample pair'),
    'fp8-rmsnorm-gemm': (1, 2, 'a CUDA kernel to a stated ABI', 'an example script checks the kernel'),
    'freecad-impeller': (1, 0, 'a parametric CAD script to stated parameters', 'no local reference'),
    'freecad-platform-drawing': (0, 0, 'geometry read from a drawing', 'no local reference'),
    'freecad-spring-clip': (1, 1, 'a parametric CAD script to stated parameters and invariants', 'stated invariants'),
    'freight-dispatch-shift': (1, 1, 'build a stateful CLI to a schema', 'a schema document'),
    'glycan-ms2-elucidation': (0, 0, 'spectrum interpretation', 'no local reference'),
    'gsea-proteomics': (0, 0, 'a statistical analysis that needs domain choices', 'no local reference'),
    'heat-pump-warranty': (1, 0, 'decisions under local rules through a portal', 'no local reference'),
    'hof-topology-interpenetration': (0, 0, 'crystal-network analysis', 'no local reference'),
    'html-js-filter': (1, 1, 'build a sanitizer to a stated contract', 'the agent can build its own cases'),
    'interleaved-vigenere': (0, 1, 'cipher identification by search', 'a worked example'),
    'intrastat-meldung': (1, 0, 'correct a staged declaration under an SOP', 'no local reference'),
    'jax-speedrun-gpu': (0, 1, 'train a model under a time budget', 'validation data'),
    'ks-solver-cpp': (0, 1, 'a numerical PDE solver', 'an oracle header'),
    'kv-live-surgery': (0, 1, 'speed up and replace a live server shipped as a binary', 'a load generator validates responses'),
    'lake-temp-glm': (0, 0, 'train a model scored on hidden profiles', 'evaluation data is hidden'),
    'layout-config-recreation': (0, 2, 'reverse-engineer a layout by search', 'a renderer and the target image'),
    'layout-config-recreation2': (0, 2, 'reverse-engineer a layout by search', 'a renderer and the target image'),
    'legacy-utility-triage': (1, 0, 'resolve cases in a GUI under a manual', 'no local reference'),
    'live-database-cutover': (1, 1, 'a zero-downtime database migration to a stated contract', 'the current backend is a behavioral baseline'),
    'math-eval-grader': (1, 2, 'an evaluation pipeline and grader', 'labeled grader examples'),
    'medical-claims-processing': (2, 2, 'repair of a rules engine', 'reference cases with expected flags'),
    'mp-checkpoint-consolidation': (1, 2, 'consolidate checkpoint shards to a stated key set', 'reference logits and expected keys ship with the task'),
    'music-harmony': (0, 0, 'harmonize a chorale from a PDF', 'no local reference'),
    'mvcc-lsm-compaction': (2, 2, 'diagnose and fix a storage defect', 'a crash reproducer and a test suite'),
    'nextjs-performance': (1, 1, 'improve web-app performance', 'latency is measurable locally'),
    'ontology-kg-querying': (1, 1, 'a pipeline and queries over RDF bundles', 'an earlier bundle as an example'),
    'payments-pipeline-fix': (2, 1, 'repair of a small worker against stated symptoms', 'the stated delivery bounds can be checked against the local broker'),
    'photonic-waveguide-routing': (0, 1, 'route optimization', 'the stated geometry rules check validity, not optimality'),
    'pretrain-shard-corruption': (2, 2, 'restore a training run against a stated symptom', 'the launcher reports the loss to reach'),
    'production-planning': (1, 1, 'a production plan through a gateway tool', 'local databases and docs'),
    'protein-autointerp-disulfide': (0, 1, 'infer an unknown feature from examples', 'training examples'),
    'react-lead-form': (2, 2, 'repair of a shared submission workflow against local specs', 'normative local specs and a submit command'),
    'retro-console-soc': (1, 2, 'build a system-on-chip to a stated interface', 'a test ROM and a simulator'),
    'risk-scorer-replay': (2, 2, 'repair of an offline evaluator', 'a black-box production command to probe'),
    'roy-polymorph-cn': (0, 0, 'fit a physical model to measurements', 'no local reference'),
    'rs-archive-clone': (1, 2, 'clone a reference binary by probing', 'the reference binary'),
    'satb-audio-transcription': (0, 0, 'transcribe audio', 'no local reference'),
    'session-window-debug': (2, 1, 'repair of stated symptoms in shipped modules', 'a design document'),
    'sglang-qwen-burst': (2, 1, 'fix a streaming-order defect', 'a baseline configuration'),
    'shadow-relay': (0, 0, 'network forensics and decryption', 'no local reference'),
    'sound-change-cascade': (0, 2, 'infer a rule cascade by search', 'a rule engine and training pairs'),
    'takens-embedding-lean': (0, 2, 'a formal proof', 'the build and an axiom audit'),
    'telecom-entity-resolution': (1, 0, 'cluster records to a stated output format', 'no labels ship with the task; quality is scored on hidden pairs'),
    'uefi-bootkit': (0, 1, 'firmware reverse engineering', 'the VM can be booted'),
    'vba-userform-port': (1, 1, 'port an app to a stated stack', 'the legacy project is the reference'),
    'vf2-speedup-networkx': (1, 2, 'a faster drop-in library', 'NetworkX is a local reference'),
    'vllm-deepseek-streaming': (2, 1, 'find and fix a streaming defect', 'reported symptoms'),
    'vpp-loss-divergence': (2, 1, 'fix framework code to match a reference trace', 'a deterministic trace'),
    'wal-recovery-ordering': (2, 1, 'repair of a storage implementation', 'a stated contract'),
    'wdm-design': (0, 1, 'inverse design of a photonic device', 'a simulator'),
}


def time_score(minutes):
    if minutes <= 10:
        return 2.0
    if minutes <= 25:
        return 1.5
    if minutes <= 45:
        return 1.0
    if minutes <= 90:
        return 0.5
    return 0.0


out = []
for task in tasks:
    E = by[task]
    low = E['low']
    allt = [t for v in E.values() for t in v]
    passes = lambda L: sum(1 for t in L if t['reward'] == 1.0)
    lp = passes(low)
    low_cost = sum(t['cost_usd'] for t in low)
    low_time = sum(dur(t) for t in low)
    fable = {
        'all_efforts': f"{passes(allt)}/{len(allt)}",
        'by_effort': {e: f"{passes(E[e])}/{len(E[e])}" for e in ['low', 'medium', 'high', 'xhigh', 'max']},
        'low_mean_trial_sec': round(low_time / len(low), 1),
        'low_mean_cost_usd': round(low_cost / len(low), 4),
        'low_cost_per_pass_usd': round(low_cost / lp, 4) if lp else None,
        'low_trial_sec_per_pass': round(low_time / lp, 1) if lp else None,
        'all_mean_trial_sec': round(st.mean(dur(t) for t in allt if t.get('started_at') and t.get('finished_at')), 1),
        'all_missing_times': sum(1 for t in allt if not (t.get('started_at') and t.get('finished_at'))),
    }
    ex = []
    if task in DEV_TEST:
        ex.append({'rule': 'R1', 'why': DEV_TEST[task]})
    if task in CAP_GAPS:
        ex.append({'rule': 'R2', 'why': 'in the capability-gap log'})
    luna = []
    if task in LUNA_BASELINE:
        luna.append('Luna baseline (#9583)')
    if task in LUNA_LABEL_ROWS:
        luna.append('a GPT-6 Luna row in the truthful-checks label set')
    if task in MICROLUNA_HOST:
        luna.append('Microluna jobs on the benchmark host')
    if task in SIGNAL_COHORT:
        luna.append('Microluna v13 trials in the #9584 prospective cohort')
    if luna:
        ex.append({'rule': 'R3', 'why': 'GPT-6 Luna trials exist: ' + '; '.join(luna)})
    sig = []
    if task in SIGNAL_CALIBRATION:
        sig.append('truthful-checks calibration half')
    if task in SIGNAL_COHORT:
        sig.append('#9584 prospective cohort')
    if task in STALL_SPLIT:
        sig.append('stall-detection calibration or evaluation split')
    if sig:
        ex.append({'rule': 'R4', 'why': 'fitted or evaluated a signal: ' + '; '.join(sig)})
    if task in TARGETED:
        ex.append({'rule': 'R4', 'why': TARGETED[task]})
    if task in ANATOMY:
        ex.append({'rule': 'R5', 'why': 'hidden tests and reference solution read for the task anatomy'})
    out.append({'task': task, 'fable': fable, 'exclusions': ex})

# R6: the task-pool grouping rule. A group is the task name's first word.
tainted_groups = defaultdict(list)
for row in out:
    if any(e['rule'] in ('R3', 'R4', 'R5') for e in row['exclusions']):
        tainted_groups[group(row['task'])].append(row['task'])
for row in out:
    g = group(row['task'])
    sibs = [t for t in tainted_groups.get(g, []) if t != row['task']]
    if sibs:
        row['exclusions'].append({'rule': 'R6', 'why': f"its group '{g}' holds {', '.join(sibs)}"})
    if row['task'] in GPU:
        row['exclusions'].append({'rule': 'R7', 'why': 'needs a GPU'})
    if row['task'] in WALL:
        row['exclusions'].append({'rule': 'R7', 'why': WALL[row['task']]})

for row in out:
    task = row['task']
    shape, check, shape_why, check_why = J[task]
    f = row['fable']
    lp = int(f['by_effort']['low'].split('/')[0])
    fl = round(2 * lp / 5, 2)
    ts = time_score(f['low_mean_trial_sec'] / 60)
    row['score'] = {'shape': shape, 'check': check, 'fable_low': fl, 'time': ts,
                    'total': round(shape + check + fl + ts, 2),
                    'shape_why': shape_why, 'check_why': check_why}
    row['luna_sized'] = shape >= 1 and check >= 1 and lp >= 4 and f['low_mean_trial_sec'] <= 45 * 60
    row['gate'] = {'untouched': not row['exclusions'], 'fable_low_reliable': lp >= 4}
    row['family'] = row['gate']['untouched'] and row['gate']['fable_low_reliable']
    row['sha256'] = hashlib.sha256(task.encode()).hexdigest()
    row['resources'] = catalog[task]['resources']

fam = [r for r in out if r['family']]
fam.sort(key=lambda r: (-r['score']['total'], r['sha256']))
for i in range(0, len(fam), 2):
    pair = fam[i:i + 2]
    pair_sorted = sorted(pair, key=lambda r: r['sha256'])
    pair_sorted[0]['split'] = 'confirmation'
    if len(pair_sorted) > 1:
        pair_sorted[1]['split'] = 'development'
    for r in pair:
        r['pair'] = i // 2 + 1
        r['rank'] = fam.index(r) + 1

for r in out:
    r.setdefault('split', None)

doc = {
    'schema': 'openagents.tbench.family-preregistration.v1',
    'experiment': '2026-09-25-luna-sized-family',
    'written': '2026-09-25',
    'sources': {
        'fable': 'bench/terminal-bench/reference/fable-5.1-replays.json',
        'fable_retrieved_at': replays['retrieved_at'],
        'task_list': 'bench/terminal-bench/reference/tb4-leaderboard.json',
        'catalog': 'bench/terminal-bench/profiles/tasks.json (catalogs.tb4, v4.0.0)',
        'signal_label_set': 'crates/coder-one/fixtures/truth/rows.jsonl',
        'stall_split': 'bench/terminal-bench/experiments/2026-09-25-stall-detection/split.json',
        'anatomy': 'docs/terminal-bench/2026-09-24-task-anatomy.json',
    },
    'policy': {
        'manifest': 'crates/coder-one/policies/microluna-v15.json',
        'name': 'coder-one-microluna-v15',
        'file_sha256': hashlib.sha256(open(ROOT + '/crates/coder-one/policies/microluna-v15.json', 'rb').read()).hexdigest(),
        'resolved_policy_digest': 'd3e1396f7b5ad7b36ec39ade8ac483dc803d375d0d844916dfd1feafe2af2ea1',
        'resolved_digest_source': 'policy_digest recorded by all seven microluna-v15 trials on 2026-09-24 (dev set and held-out test set)',
        'artifact': 'coder-one 0.1.0 (83b48ccc08c8)',
        'artifact_commit': '83b48ccc08c8f3ed7f5d25e41594e2efb348d105',
    },
    'score_rule': {
        'shape': '2 repair of shipped source against stated symptoms or a normative local spec; 1 build or engineering to a clear specification; 0 search, inference, perception, proof, open optimization, or outside domain judgment',
        'check': '2 an exact local reference, a reproducer, or runnable tests or examples ship with the task; 1 the stated criteria can be checked with a harness built from local tools; 0 no local ground truth',
        'fable_low': '2 x (Fable 5.1 low passes / 5)',
        'time': 'Fable 5.1 low mean trial time: <=10 min 2, <=25 min 1.5, <=45 min 1, <=90 min 0.5, else 0',
        'total': 'sum, 0 to 8',
        'luna_sized': 'shape >= 1, check >= 1, Fable low >= 4 of 5, and Fable low mean trial time <= 45 min',
    },
    'family_rule': 'untouched (no exclusion rule applies) and Fable 5.1 low passes at least 4 of 5',
    'split_rule': 'order the family by total score, descending, ties by SHA-256 of the task id ascending; pair ranks 1-2, 3-4, 5-6; within each pair the task whose SHA-256 of its id (UTF-8, no newline) is smaller is confirmation, the other development',
    'family': [r['task'] for r in fam],
    'development': [r['task'] for r in fam if r['split'] == 'development'],
    'confirmation': [r['task'] for r in fam if r['split'] == 'confirmation'],
    'preregistration': {
        'protocol': 'bench/terminal-bench/experiments/2026-09-25-luna-sized-family/protocol.md',
        'attempts_per_task': 3,
        'counted_trials': 3 * len(fam),
        'cheap_win': 'at least 2 of 3 attempts pass, and the cost per pass (Luna and Jev, every counted attempt) is under 10% of Fable 5.1 low cost per pass on the task',
        'win': 'at least 2 of the 3 confirmation tasks are cheap wins',
        'loss': 'no confirmation task passes 2 of 3, and at most 1 of the 9 confirmation attempts passes',
        'inconclusive': 'any other result, or an invalid run',
        'cost_ratio_max': 0.10,
        'cost_ceiling_usd': 3.00,
        'time_comparison': 'trial time per pass against Fable 5.1 low trial time per pass; reported, not part of the win',
    },
    'tasks': out,
}
json.dump(doc, open(OUT, 'w'), indent=2)
print('family', [(r['rank'], r['task'], r['score']['total'], r['split'], r['luna_sized']) for r in fam])
print('eligible not reliable', [(r['task'], r['fable']['by_effort']['low']) for r in out if r['gate']['untouched'] and not r['family']])
print('luna_sized all', [(r['task'], bool(r['exclusions'])) for r in out if r['luna_sized']])
