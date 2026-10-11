"""Fixed paths and metadata checks for a third-runtime diagnostic."""
import hashlib
import json
import os
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
ISOLATION = HERE.parent
RUN = ISOLATION.parent
BASE = RUN.parent
ROOT = Path('@@CLEF_WORKTREE@@')
PYTHON = Path('@@CLEF_PYTHON_ENV@@/bin/python')
UPSTREAM = BASE / 'q4-down-q6-regeneration/upstream-checkout'
REVISION = '62a6f74bab748b1755f4cd8dfc0871caf865c6c8'
M1_TAG = '79e2e74eb11022c1ba2e438df7f0ca2d4c10f8b6'
GGUF = BASE / 'artifacts/Clef-Q4_K_M.gguf'
GGUF_SHA = '9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822'
ENCODED = RUN / 'encoder-source-run-1/native-encoded.json'
CPU = ISOLATION / 'cpu-1/results/receipt.json'

def require(condition, message):
    if not condition:
        raise ValueError(message)

def sha(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024**2), b''):
            h.update(block)
    return h.hexdigest()

def write(path, value):
    part = path.with_suffix(path.suffix + '.partial')
    part.write_text(json.dumps(value, indent=2, allow_nan=False) + '\n')
    part.replace(path)

def state(path):
    s = path.stat()
    return [s.st_size, s.st_ino, s.st_mtime_ns, s.st_ctime_ns]

def upstream_identity():
    def git(*args):
        return subprocess.check_output(['git', '-C', str(UPSTREAM), *args], timeout=20, text=True).strip()
    require(git('rev-parse', 'HEAD') == REVISION, 'upstream revision changed')
    require(git('status', '--porcelain', '--untracked-files=all') == '', 'upstream source is not clean')
    return {'revision': REVISION, 'tree': git('rev-parse', 'HEAD^{tree}'),
            'M1_b11538_tag_commit': M1_TAG, 'is_exact_M1_version': False,
            'M1_source_complete_locally': False}

def frozen_input():
    require(sha(ENCODED) == '5c7e9cc9c02e78044d17adc1fd5a8816919e62ee13b32eb6bb0e463a18e4ce81', 'encoder proof changed')
    cpu = json.loads(CPU.read_text())
    require(cpu['completed'] and len(cpu['layers']) == 64, 'independent CPU capture incomplete')
    encoded = next(row for row in json.loads(ENCODED.read_text())['cases'] if row['id'] == 'text-029')
    record = cpu['records'][0]
    require(encoded['request_sha256'] == cpu['request_sha256'] == 'd97a363d10a51e9bfd42dbb8f7f5ae01f3a873ee9ea2589c3a0faa54cf6efdad', 'request identity changed')
    require(encoded['input_ids'] == record['input_ids'] == cpu['input_ids'] and len(cpu['input_ids']) == 359, 'exact token IDs differ')
    require(encoded['truncated_state_tokens'] == 0, 'unexpected truncation')
    require(len(record['questions']) == len(encoded['questions']) == 3 and record['media'] is None, 'question/media shape differs')
    marks = [0] * 359
    for q, e in zip(record['questions'], encoded['questions']):
        require({'id': q['question_id'], 'type': q['question_type'], 'question_span': q['question_span'],
                 'option_spans': q['option_spans'], 'option_ids': q['option_ids']} == e, 'native and official spans/options differ')
        require(type(q['question_type']) is int and 0 <= q['question_type'] <= 2, 'invalid question type')
        for span, mark in [(q['question_span'], q['question_type'] + 1), *[(s, 4) for s in q['option_spans']]]:
            first, end = span
            require(0 < first < end < 359 and all(v == 0 for v in marks[first-1:end+1]), 'overlapping or unseparated spans')
            marks[first:end] = [mark] * (end - first)
    require(record['questions'][1]['question_id'] == '日本1' and record['questions'][1]['option_ids'] == ['true', 'false'], 'diagnostic question changed')
    require(state(GGUF) == cpu['gguf_file_state'], 'GGUF differs from independently decoded snapshot')
    text = ''.join(f'{i}\t{token}\t{marks[i]}\n' for i, token in enumerate(cpu['input_ids']))
    return {'id': 'text-029', 'request_sha256': cpu['request_sha256'], 'record': record,
            'input_tsv_sha256': hashlib.sha256(text.encode()).hexdigest(), 'input_tsv': text,
            'gguf_state': cpu['gguf_file_state'], 'gguf_sha256_from_prior_verification': GGUF_SHA,
            'native_id_limit': 'Stage6 HTTP did not export IDs. Exact production parser/encoder source reconstruction matched these independent IDs; this does not add IDs to the historical HTTP response.'}

def clean_environment():
    env = dict(os.environ)
    for key in list(env):
        if key.startswith(('DYLD_', 'GGML_', 'LLAMA_', 'PYTORCH_MPS_')) or key in ('LD_PRELOAD', 'LD_LIBRARY_PATH'):
            del env[key]
    env.update(CLEF_LLAMA_GUARDED='1', OMP_NUM_THREADS='2', OPENBLAS_NUM_THREADS='2', MKL_NUM_THREADS='2',
               VECLIB_MAXIMUM_THREADS='2', RAYON_NUM_THREADS='2', PYTHONDONTWRITEBYTECODE='1',
               CUDA_VISIBLE_DEVICES='', HF_HUB_OFFLINE='1', TRANSFORMERS_OFFLINE='1',
               PYTORCH_ENABLE_MPS_FALLBACK='0', CMAKE_BUILD_PARALLEL_LEVEL='2')
    return env
