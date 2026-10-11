"""Run one already-built CPU capture with fixed token IDs and span markers."""
import argparse
import json
import os
from pathlib import Path
import subprocess
from common import CPU, GGUF, HERE, frozen_input, require, sha, state, upstream_identity, write

def main():
    p = argparse.ArgumentParser()
    p.add_argument('--output-dir', type=Path, required=True)
    p.add_argument('--build-receipt', type=Path, required=True)
    a = p.parse_args()
    require(os.environ.get('CLEF_LLAMA_GUARDED') == '1', 'owned capture guard required')
    require(not a.output_dir.exists(), 'preserve previous output')
    a.output_dir.mkdir(parents=True)
    built = json.loads(a.build_receipt.read_text())
    require(built['status'] == 'complete' and built['upstream'] == upstream_identity(), 'build identity differs')
    binary = Path(built['binary'])
    require(HERE in binary.resolve().parents and sha(binary) == built['binary_sha256'], 'binary identity differs')
    input_data = frozen_input()
    text = input_data.pop('input_tsv')
    tokens = a.output_dir / 'input.tsv'
    with tokens.open('x') as stream:
        stream.write(text)
    require(sha(tokens) == input_data['input_tsv_sha256'], 'exact input bytes differ')
    rows = a.output_dir / 'rows'
    rows.mkdir()
    result = {'schema': 'clef.text029-llama-trunk.v1', 'status': 'running', 'completed': False,
              'diagnostic_only': True, 'upstream': built['upstream'], 'build_receipt_sha256': sha(a.build_receipt),
              'binary_sha256': sha(binary), 'independent_cpu_receipt_sha256': sha(CPU), **input_data,
              'scope': 'Third-runtime trunk comparison. The original Torch full40/104 gate remains unchanged.',
              'embedded_head_scope': 'Runs as part of the upstream graph, but its quantized weights differ from the external official head. Its logits are excluded from head-equivalent comparisons.'}
    write(a.output_dir / 'receipt.json', result)
    try:
        subprocess.run([str(binary), str(GGUF), str(tokens), str(rows)], check=True, timeout=590)
        names = [f'l_out-{i}.f32' for i in range(64)] + ['result_norm.f32']
        require({x.name for x in rows.iterdir()} == set(names) | {'driver.json'}, 'unexpected capture files')
        files = []
        for name in names:
            f = rows / name
            require(f.stat().st_size == 359 * 5120 * 4, 'capture size differs')
            files.append({'name': name, 'bytes': f.stat().st_size, 'sha256': sha(f)})
        driver = json.loads((rows / 'driver.json').read_text())
        require(driver['status'] == 'complete' and driver['captures'] == 65 and not driver['gpu_executed']
                and not driver['cache'] and driver['embedded_head_excluded_from_comparison'], 'driver scope or coverage differs')
        require(sha(binary) == built['binary_sha256'] and state(GGUF) == input_data['gguf_state'], 'binary or GGUF changed')
        require(upstream_identity() == built['upstream'], 'upstream changed during capture')
        result.update(status='complete', completed=True, files=files, driver=driver,
                      driver_receipt_sha256=sha(rows / 'driver.json'), gguf_unchanged=True)
    except BaseException as error:
        result.update(status='failed', error_type=type(error).__name__, error=str(error))
        raise
    finally:
        write(a.output_dir / 'receipt.json', result)

if __name__ == '__main__':
    main()
