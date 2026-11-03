#!/usr/bin/env python3
# Firm reason: 2 (independent original-reference tensor/token/cache comparison).
"""Compare the native Coax stream with already-recorded original-weight stages."""
import argparse
import ctypes as C
import hashlib
import json
import itertools
import pathlib
import time
import wave
import unicodedata


def normalized(text):
    text = unicodedata.normalize('NFKC',text).casefold()
    return ' '.join(''.join(c if unicodedata.category(c)[0] in 'LNM' or c == "'" else ' ' for c in text).split())


def distance(a,b):
    previous = list(range(len(b)+1))
    for i,x in enumerate(a,1):
        current = [i]
        for j,y in enumerate(b,1):
            current.append(min(current[-1]+1,previous[j]+1,previous[j-1]+(x!=y)))
        previous = current
    return previous[-1]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('library', type=pathlib.Path)
    ap.add_argument('model', type=pathlib.Path)
    ap.add_argument('reference', type=pathlib.Path)
    ap.add_argument('out', type=pathlib.Path)
    ap.add_argument('--partition', type=int, default=160)
    ap.add_argument('--only')
    ap.add_argument('--corpus',type=pathlib.Path)
    a = ap.parse_args()
    corpus = {r['id']:r for r in json.loads(a.corpus.read_text())} if a.corpus else {}
    import torch  # establishes the installed numerical libraries on NixOS
    import numpy as np
    assert 1 <= a.partition <= 2560
    lib = C.CDLL(str(a.library.resolve()))
    signatures = {
        'open': ([C.c_char_p, C.c_void_p, C.c_int64], C.c_void_p),
        'close': ([C.c_void_p], None), 'reset': ([C.c_void_p, C.c_uint64], C.c_int),
        'push': ([C.c_void_p, C.c_uint64, C.c_uint64, C.c_void_p, C.c_int64, C.c_int], C.c_int),
        'info': ([C.c_void_p, C.c_void_p], C.c_int),
        'memory': ([C.c_void_p, C.c_void_p], C.c_int),
        'inspect': ([C.c_void_p, C.c_void_p, C.c_void_p], C.c_int),
        'features': ([C.c_void_p, C.c_void_p], C.c_int),
        'cache': ([C.c_void_p, C.c_int, C.c_void_p, C.c_void_p], C.c_int),
        'text': ([C.c_void_p, C.c_uint64, C.c_void_p, C.c_int64], C.c_int64),
        'tokens': ([C.c_void_p, C.c_void_p, C.c_void_p, C.c_int64], C.c_int64),
        'error': ([C.c_void_p], C.c_char_p),
    }
    for name, (args, result) in signatures.items():
        fn = getattr(lib, 'psionic_incremental_' + name)
        fn.argtypes, fn.restype = args, result
        signatures[name] = fn
    error = C.create_string_buffer(512)
    start = time.monotonic()
    engine = signatures['open'](str(a.model).encode(), error, len(error))
    assert engine, error.value
    record = {'schema': 'psionic.incremental-product-comparison.v1',
        'library_sha256': hashlib.sha256(a.library.read_bytes()).hexdigest(),
        'partition': a.partition, 'open_seconds': time.monotonic() - start, 'clips': []}
    memory = np.zeros(4,dtype=np.int64)
    assert signatures['memory'](engine,memory.ctypes.data)
    record['gpu_allocated_bytes'] = memory.tolist()
    try:
        reference = json.loads((a.reference / 'record.json').read_text())
        for epoch, row in enumerate(reference['clips'], 1):
            if a.only and row['id'] != a.only:
                continue
            assert signatures['reset'](engine, epoch)
            if row.get('input_format') == 'wav':
                with wave.open(row['input'],'rb') as f: raw = f.readframes(f.getnframes())
            else:
                raw = pathlib.Path(row['input']).read_bytes()
            assert hashlib.sha256(raw).hexdigest() == row['pcm_sha256'], 'reference input changed'
            audio = np.frombuffer(raw,dtype='<i2').astype(np.float32) / 32768
            folder = a.reference / row['id']
            encoded = np.fromfile(folder / 'encoded.f32', dtype='<f4').reshape(-1, 2, 1024)
            projected = np.fromfile(folder / 'projected.f32', dtype='<f4').reshape(-1, 2, 640)
            features = np.fromfile(folder / 'features.f32', dtype='<f4').reshape(-1, 128)
            info = np.zeros(6, dtype=np.int64)
            enc = np.zeros((2, 1024), dtype=np.float32)
            proj = np.zeros((2, 640), dtype=np.float32)
            feat = np.zeros((16, 128), dtype=np.float32)
            key = np.zeros((70, 1024), dtype=np.float32)
            conv = np.zeros((8, 1024), dtype=np.float32)
            tokens = np.zeros(4096, dtype=np.int64)
            positions = np.zeros(4096, dtype=np.int64)
            text = C.create_string_buffer(8000)
            seq = 0
            chunks = 0
            worst = {}
            chunk_times = []
            first_partial = None
            def compare(name, got, expected):
                difference = np.abs(got - expected)
                tolerance = .001 + .001 * np.abs(expected)
                assert bool(np.isfinite(got).all()) and bool((difference <= tolerance).all()), (
                    row['id'], chunks, name, float(difference.max()), int((difference > tolerance).sum()))
                worst[name] = max(worst.get(name, 0), float((difference / tolerance).max()))
            started = time.monotonic()
            for at in itertools.chain(range(0, len(audio), a.partition), [len(audio)]):
                final = at >= len(audio)
                part = audio[at:min(at + a.partition, len(audio))]
                before = time.monotonic()
                status = signatures['push'](engine, epoch, seq, part.ctypes.data, len(part), int(final))
                elapsed = time.monotonic() - before
                if final:
                    final_seconds = elapsed
                assert status == 0, (status, signatures['error'](engine))
                seq += 1
                assert signatures['info'](engine, info.ctypes.data)
                if info[3] != chunks:
                    assert info[3] == chunks + 1
                    chunks += 1
                    chunk_times.append(elapsed)
                    assert signatures['inspect'](engine, enc.ctypes.data, proj.ctypes.data)
                    compare('encoder', enc, encoded[chunks-1])
                    compare('projected', proj, projected[chunks-1])
                    n = signatures['features'](engine, feat.ctypes.data)
                    offset = 0 if chunks == 1 else 9 + (chunks-2)*16
                    expected = np.zeros((n,128), dtype=np.float32)
                    valid = features[offset:offset+n]
                    expected[:len(valid)] = valid
                    compare('features', feat[:n], expected)
                    for layer in (0,23):
                        path = folder / f'cache-{chunks}-{layer}-keys.f32'
                        if path.exists():
                            nkey = signatures['cache'](engine, layer, key.ctypes.data, conv.ctypes.data)
                            expected = np.fromfile(path,dtype='<f4').reshape(8,nkey,128).transpose(1,0,2).reshape(nkey,1024)
                            compare('keys', key[:nkey], expected)
                            expected = np.fromfile(folder / f'cache-{chunks}-{layer}-conv.f32',dtype='<f4').reshape(1024,8).T
                            compare('convolution', conv, expected)
                    ntext = signatures['text'](engine, epoch, text, len(text))
                    assert ntext >= 0
                    assert text.raw[:ntext].decode() == row['chunks'][chunks-1]['text'], (row['id'],chunks,'partial text')
                    if ntext and first_partial is None:
                        first_partial = {'audio_samples_received': int(info[1]), 'wall_seconds': time.monotonic()-started}
                if final:
                    break
            count = signatures['tokens'](engine,tokens.ctypes.data,positions.ctypes.data,len(tokens))
            ntext = signatures['text'](engine,epoch,text,len(text))
            got_text = text.raw[:ntext].decode()
            assert tokens[:count].tolist() == row['tokens'], (row['id'], tokens[:count].tolist(), row['tokens'])
            assert positions[:count].tolist() == row['positions']
            assert got_text == row['text'], (got_text,row['text'])
            result = {'id': row['id'], 'text': got_text, 'tokens': count, 'chunks': chunks,
                'seconds': time.monotonic()-started, 'worst_tolerance_fraction': worst,
                'chunk_seconds': chunk_times, 'first_partial': first_partial,
                'final_seconds': final_seconds, 'passed': True}
            if row['id'] in corpus:
                wanted = normalized(corpus[row['id']]['reference']);got = normalized(got_text)
                ww,gw = wanted.split(),got.split()
                result['quality'] = {'set':corpus[row['id']]['set'],'reference':wanted,
                    'words':len(ww),'word_errors':distance(ww,gw),'characters':len(wanted),
                    'character_errors':distance(wanted,got),'no_speech_false_positive':not ww and bool(gw),
                    'first_word_exact':bool(ww and gw and ww[0]==gw[0]),
                    'last_word_exact':bool(ww and gw and ww[-1]==gw[-1])}
            assert signatures['memory'](engine,memory.ctypes.data)
            assert memory.tolist() == record['gpu_allocated_bytes'], 'stream grew GPU allocations'
            record['clips'].append(result)
            print(row['id'], chunks, got_text, worst, flush=True)
    finally:
        signatures['close'](engine)
    a.out.parent.mkdir(parents=True,exist_ok=True)
    a.out.write_text(json.dumps(record,indent=2)+'\n')


if __name__ == '__main__':
    main()
