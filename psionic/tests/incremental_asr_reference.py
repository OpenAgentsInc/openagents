#!/usr/bin/env python3
# Firm reason: 3 (pinned Transformers original-weight streaming reference; no shipped Python).
"""Record independent cache-aware stages before the Coax implementation runs."""
import argparse
import hashlib
import json
import pathlib
import sys
import time
import wave


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('model', type=pathlib.Path)
    ap.add_argument('source', type=pathlib.Path)
    ap.add_argument('out', type=pathlib.Path)
    ap.add_argument('clips', nargs='+', type=pathlib.Path)
    ap.add_argument('--offload', action='store_true', help='reference-only layer transfers; all arithmetic still CUDA')
    a = ap.parse_args()
    # Import the already-installed numerical dependency first (also establishes
    # its shared-library search on NixOS), then the reviewed pinned source tree.
    import torch
    sys.path.insert(0, str(a.source / 'src'))
    import numpy as np
    import transformers
    from transformers import NemotronAsrStreamingForRNNT
    from transformers.models.nemotron_asr_streaming.feature_extraction_nemotron_asr_streaming import NemotronAsrStreamingFeatureExtractor
    from transformers.models.nemotron_asr_streaming.generation_nemotron_asr_streaming import NemotronAsrStreamingRNNTDecoderCache
    import transformers.models.nemotron_asr_streaming.modeling_nemotron_asr_streaming as modeling
    from tokenizers import Tokenizer
    torch.set_num_threads(4)
    torch.backends.cuda.matmul.allow_tf32 = False
    torch.backends.cudnn.allow_tf32 = False
    torch.use_deterministic_algorithms(True)
    a.out.mkdir(parents=True, exist_ok=True)
    model = NemotronAsrStreamingForRNNT.from_pretrained(a.model, local_files_only=True,
        dtype=torch.float32, attn_implementation='eager').eval()
    if a.offload:
        for module in (model.encoder.subsampling, model.encoder.encode_positions,
                       model.encoder_projector, model.decoder, model.joint):
            module.cuda()
        for layer in model.encoder.layers:
            cpu = {name: p.detach() for name,p in layer.named_parameters()}
            def before(module,args): module.cuda()
            def after(module,args,out,cpu=cpu):
                for name,p in module.named_parameters(): p.data = cpu[name]
                torch.cuda.empty_cache()
            layer.register_forward_pre_hook(before)
            layer.register_forward_hook(after)
    else:
        model.cuda()
    # The pinned eager module applies logical_not to its attention mask and
    # therefore requires True=allowed. Its generic mask factory currently
    # returns additive 0/-inf. Adapt the representation only, retaining every
    # allowed/disallowed bit. Without this adapter the first chunk is NaN.
    original_mask = modeling.create_bidirectional_mask
    def bool_mask(*args, **kwargs):
        mask = original_mask(*args, **kwargs)
        if mask is not None and mask.dtype != torch.bool:
            assert bool(((mask == 0) | (mask <= -1e30)).all())
            mask = mask == 0
        return mask
    modeling.create_bidirectional_mask = bool_mask
    cfg = json.loads((a.model / 'processor_config.json').read_text())['feature_extractor']
    cfg.pop('feature_extractor_type')
    extractor = NemotronAsrStreamingFeatureExtractor(**cfg)
    tokenizer = Tokenizer.from_file(str(a.model / 'tokenizer.json'))
    record = {'schema': 'psionic.incremental-reference.v1', 'source_revision':
        '9719c27f4e04e8d10ed1f908bdb3712d16fef73c', 'source_root': str(a.source),
        'torch': torch.__version__, 'transformers': transformers.__version__,
        'device': torch.cuda.get_device_name(), 'tf32': False, 'lookahead': 1,
        'reference_weight_offload': a.offload,
        'profile': 'psionic-nemotron-streaming-en-v1', 'clips': [],
        'reference_adapter': 'Generic mask-factory additive 0/negative-limit mask -> boolean allowed mask, before both padding-row detection and attention. No numerical/model/state changes. Unadapted first chunk yields NaNs; attention-only adaptation incorrectly zeros convolution input and yields all blanks.',
        'final_padding': 'Zero mel frames to 9 first / 16 later; same fixed-size streaming encoder reference; all returned frames decoded.'}
    srcdir = a.source / 'src/transformers/models/nemotron_asr_streaming'
    record['source_sha256'] = {p.name: sha(p) for p in sorted(srcdir.glob('*.py'))}
    with torch.inference_mode():
        for pcm in a.clips:
            if pcm.suffix == '.wav':
                with wave.open(str(pcm), 'rb') as f:
                    assert (f.getframerate(),f.getnchannels(),f.getsampwidth()) == (16000,1,2)
                    raw = f.readframes(f.getnframes())
            else:
                raw = pcm.read_bytes()
            audio = np.frombuffer(raw, dtype='<i2').astype(np.float32) / 32768
            features = extractor(audio, sampling_rate=16000, return_tensors='pt')['input_features'].cuda()
            frames = len(audio) // 160
            features = features[:, :frames]
            folder = a.out / pcm.stem
            folder.mkdir(exist_ok=True)
            features.cpu().numpy().astype('<f4').tofile(folder / 'features.f32')
            cache = padding = None
            decoder_cache = NemotronAsrStreamingRNNTDecoderCache(model.config)
            prediction = model.decoder(torch.tensor([[1024]], device='cuda'), cache=decoder_cache)
            tokens, positions, chunks, encoded, projected, partials = [], [], [], [], [], []
            at = 0
            torch.cuda.synchronize()
            start = time.monotonic()
            while at < frames:
                need = 9 if at == 0 else 16
                chunk = features[:, at:min(at + need, frames)]
                if chunk.shape[1] < need:
                    chunk = torch.nn.functional.pad(chunk, (0, 0, 0, need - chunk.shape[1]))
                before = time.monotonic()
                out = model.encoder(chunk, past_key_values=cache, padding_cache=padding,
                    use_cache=True, num_lookahead_tokens=1, output_attention_mask=False)
                cache, padding = out.past_key_values, out.padding_cache
                enc = out.last_hidden_state
                assert bool(torch.isfinite(enc).all()), 'nonfinite reference encoder'
                proj = model.encoder_projector(enc)
                encoded.append(enc.cpu().numpy())
                projected.append(proj.cpu().numpy())
                for t in range(proj.shape[1]):
                    for _ in range(10):
                        logits = model.joint(prediction, proj[:, t:t+1])
                        assert bool(torch.isfinite(logits).all()), 'nonfinite reference logits'
                        token = int(logits[0, 0].argmax())
                        if token == 1024:
                            break
                        tokens.append(token)
                        positions.append(len(chunks) * 2 + t)
                        prediction = model.decoder(torch.tensor([[token]], device='cuda'), cache=decoder_cache)
                torch.cuda.synchronize()
                partials.append(tokenizer.decode(tokens, skip_special_tokens=True))
                chunks.append({'feature_offset': at, 'frames': need, 'seconds': time.monotonic() - before,
                               'tokens': len(tokens), 'text': partials[-1]})
                # Original cache values at both initial and saturated boundaries.
                if len(chunks) in (1, 2, 36):
                    for k in (0, 23):
                        cache.layers[k].keys.cpu().numpy().astype('<f4').tofile(folder / f'cache-{len(chunks)}-{k}-keys.f32')
                        padding.layers[f'conv.{k}'].cache.cpu().numpy().astype('<f4').tofile(folder / f'cache-{len(chunks)}-{k}-conv.f32')
                at += need
            elapsed = time.monotonic() - start
            np.concatenate(encoded, axis=1).astype('<f4').tofile(folder / 'encoded.f32')
            np.concatenate(projected, axis=1).astype('<f4').tofile(folder / 'projected.f32')
            row = {'id': pcm.stem, 'input': str(pcm), 'pcm_sha256': hashlib.sha256(raw).hexdigest(),
                   'input_format': 'wav' if pcm.suffix == '.wav' else 'pcm',
                   'samples': len(audio), 'feature_frames': frames, 'tokens': tokens, 'positions': positions,
                   'text': partials[-1], 'chunks': chunks, 'seconds': elapsed,
                   'artifacts': {p.name: sha(p) for p in sorted(folder.glob('*.f32'))}}
            (folder / 'result.json').write_text(json.dumps(row, indent=2) + '\n')
            record['clips'].append(row)
            print(pcm.stem, len(chunks), len(tokens), row['text'], flush=True)
    (a.out / 'record.json').write_text(json.dumps(record, indent=2) + '\n')


if __name__ == '__main__':
    main()
