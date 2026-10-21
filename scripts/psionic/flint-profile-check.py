#!/usr/bin/env python3
"""Qualify the bounded Flint Pylon profile on its dedicated GPU host."""
import argparse
import concurrent.futures
import http.client
import json
import re
import subprocess
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--url', default='http://127.0.0.1:8080')
p.add_argument('--binary', type=Path, required=True)
p.add_argument('--weights', type=Path, required=True)
p.add_argument('--output', type=Path, required=True)
a = p.parse_args()
rows = []
memory = []
stop = threading.Event()

def measure():
    while not stop.is_set():
        try:
            value = subprocess.check_output(['nvidia-smi', '--query-compute-apps=used_memory', '--format=csv,noheader,nounits'], text=True)
            memory.append(sum(int(n) for n in value.split() if n.isdigit()))
        except subprocess.CalledProcessError:
            pass
        stop.wait(.5)


def info():
    with urllib.request.urlopen(a.url + '/v1/models', timeout=10) as response:
        return json.load(response)


def call(prompt='', messages=None, **options):
    body = dict(model='flint', messages=messages or [dict(role='user', content=prompt)],
                max_tokens=6144, temperature=1, top_p=.95, top_k=20, seed=42)
    body.update(options)
    request = urllib.request.Request(a.url + '/v1/chat/completions', data=json.dumps(body).encode(), headers={'Content-Type':'application/json'})
    start = time.monotonic()
    try:
        with urllib.request.urlopen(request, timeout=920) as response:
            result = dict(status=response.status, body=json.load(response))
    except urllib.error.HTTPError as error:
        result = dict(status=error.code, body=json.load(error))
    result['seconds'] = time.monotonic() - start
    return result


def finished(name, result, expected=None):
    rows.append(dict(check=name, result=result))
    assert result['status'] == 200, result
    choice = result['body']['choices'][0]
    assert choice['finish_reason'] == 'stop', result
    text = choice['message']['content'].strip()
    assert text and '<think>' not in text and '</think>' not in text, result
    assert expected is None or text.replace(',', '').strip(' \n`*.$\"\'') == expected, result
    assert result['seconds'] < 900
    a.output.write_text(json.dumps(dict(checks=rows, maximum_sampled_process_memory_mib=max(memory, default=0)), indent=2)+'\n')
    print(name, 'passed', result['body']['usage'], round(result['seconds'],2), flush=True)

thread = threading.Thread(target=measure, daemon=True)
thread.start()
try:
    assert info()['psionic']['max_context_tokens'] == 8192
    cases = [('arithmetic', 'What is 19 + 23? Reply with just the number.', '42'),
             ('spanish', 'Responde en español: ¿cuánto es 7 por 8? Escribe solo el número.', '56'),
             ('reasoning', 'What is 2 to the power 20? Reply with the exact integer.', '1048576')]
    for name, prompt, expected in cases:
        finished(name, call(prompt), expected)
    finished('history', call(messages=[dict(role='user',content='Remember that N is 37.'),
        dict(role='assistant',content='N is 37.'),dict(role='user',content='What is N + 5? Reply with just the number.')]), '42')
    code = call('Write a Rust function named add that takes two i32 values and returns their sum. Return only the function, without explanations.')
    finished('code', code)
    text = code['body']['choices'][0]['message']['content']
    match = re.search(r'```(?:rust)?\s*(.*?)```', text, re.S)
    source = match.group(1) if match else text
    code_path = a.output.with_suffix('.rs')
    code_path.write_text(source + '\nfn main(){assert_eq!(add(2,2),4);assert_eq!(add(-7,3),-4);assert_eq!(add(0,0),0);assert_eq!(add(19,23),42);}\n')
    subprocess.run(['rustc', str(code_path), '-o', str(a.output.with_suffix('.code-test'))], check=True)
    subprocess.run([str(a.output.with_suffix('.code-test'))], check=True)

    token_request = a.output.with_suffix('.token-request.json')
    def tokens(prompt):
        token_request.write_text(json.dumps(dict(model='flint',messages=[dict(role='user',content=prompt)])))
        return len(json.loads(subprocess.check_output([str(a.binary),'tokenize','--weights',str(a.weights),'--request',str(token_request)]))['token_ids'])
    def long_prompt(n):
        return 'The secret word is FLINTCLOUD42.\n' + 'A red flag means stop and a green flag means go.\n' * n + 'What is the secret word? Return only that word.'
    low, high = 0, 1000
    while low < high:
        middle = (low + high + 1)//2
        if tokens(long_prompt(middle)) <= 2048: low = middle
        else: high = middle-1
    prompt = long_prompt(low)
    assert 2000 <= tokens(prompt) <= 2048
    finished('maximum_prompt_profile', call(prompt), 'FLINTCLOUD42')
    stress = long_prompt(low-5).rsplit('What is the secret word?',1)[0] + 'Write a Markdown table with every integer from 1 to 1000 and its square and cube. Write every row explicitly. Do not use code, ellipses, or abbreviations.'
    while tokens(stress) > 2048:
        stress = stress.replace('A red flag means stop and a green flag means go.\n','',1)
    assert tokens(stress) >= 1800
    exhausted = call(stress)
    rows.append(dict(check='output_ceiling',result=exhausted))
    assert exhausted['status'] == 200, exhausted
    assert exhausted['body']['choices'][0]['finish_reason'] == 'length', exhausted
    assert exhausted['body']['usage']['completion_tokens'] == 6144, exhausted
    assert exhausted['body']['usage']['total_tokens'] >= 7944, exhausted
    print('output_ceiling passed', exhausted['body']['usage'], round(exhausted['seconds'],2), flush=True)
    rejected = call(long_prompt(low + 50))
    rows.append(dict(check='context_refusal',result=rejected))
    assert rejected['status'] == 400 and rejected['body']['error']['code'] == 'context_length_exceeded', rejected

    for i in range(12):
        finished(f'sustained_{i+1}', call('What is 2 + 2? Reply with just the number.'), '4')

    address = urllib.parse.urlsplit(a.url)
    connection = http.client.HTTPConnection(address.hostname, address.port, timeout=10)
    connection.request('POST','/v1/chat/completions',json.dumps(dict(model='flint',messages=[dict(role='user',content='Write 150 separate numbered lines explaining why software tests matter.')],max_tokens=2048,temperature=0)),{'Content-Type':'application/json'})
    for _ in range(100):
        if info()['psionic']['busy']: break
        time.sleep(.05)
    else: raise AssertionError('generation did not acquire its slot')
    connection.close()
    busy = call('Hello')
    assert busy['status'] == 429, busy
    assert info()['psionic']['busy']
    until = time.monotonic()+920
    while info()['psionic']['busy']:
        assert time.monotonic() < until
        time.sleep(1)
    rows.append(dict(check='disconnect_keeps_generation_owned',result=busy))
    finished('after_disconnect', call('What is 2 + 2? Reply with just the number.'), '4')
finally:
    stop.set()
    thread.join(timeout=5)
    a.output.write_text(json.dumps(dict(checks=rows, maximum_sampled_process_memory_mib=max(memory,default=0)),indent=2)+'\n')
