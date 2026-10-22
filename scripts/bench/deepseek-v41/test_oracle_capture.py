"""Read-only stage-input capture controls; run with the oracle's PyTorch."""
import types
import unittest

import oracle_dump as dump

try:
    import torch
except ImportError:
    torch = None


def config():
    return types.SimpleNamespace(
        kv_source_layers=[2, 8, 20, 32],
        index_source_layers=[2, 8, 14, 20, 24, 28, 32, 36],
        compress_ratios=[0] * 8 + [2] * 12 + [1] * 20,
        candidate_source_layer=20, window_size=128, index_topk=4,
        candidate_block_size=2,
    )


class Owners(unittest.TestCase):
    def test_original_four_stage_boundaries(self):
        for layer, kv, keys, selection, candidates in [
            (10, 8, True, 8, False),
            (20, None, False, None, False),
            (30, 20, False, 28, True),
        ]:
            self.assertEqual(dump.boundary_sources(layer, config()), {
                'kv_source': kv, 'index_keys': keys,
                'selection_source': selection, 'candidates': candidates,
            })


@unittest.skipIf(torch is None, 'requires the oracle PyTorch environment')
class Captures(unittest.TestCase):
    def inputs(self, start, tokens):
        x = torch.arange(tokens * 8).reshape(1, tokens, 4, 2).to(torch.bfloat16)
        pre = torch.ones(1, tokens, 4, dtype=torch.float32)
        offset = tokens if start == 0 else 128
        shared = types.SimpleNamespace(
            compress_kv=torch.arange(1024 * 2).reshape(1, 1024, 2).to(torch.bfloat16),
            index_k=torch.arange(1024 * 4).reshape(1, 1024, 4).to(torch.bfloat16),
            topk_idxs=torch.tensor([[[offset, offset + 1, -1]]] * tokens).reshape(1, tokens, 3),
            candidates=torch.ones(1, tokens, 8, dtype=torch.bool),
        )
        rec = dump.Recorder(0)
        rec.stage_boundaries = {10, 20, 30}
        rec.begin('c', 'decode', 0 if start == 0 else 1, [], True)
        return rec, (x, start, pre), shared

    def test_full_prefill_and_decode_keep_only_visible_shared_rows(self):
        for start, tokens in [(0, 17), (1001, 1)]:
            rec, args, shared = self.inputs(start, tokens)
            before = {k: v.clone() for k, v in vars(shared).items()}
            dump.capture_boundary(rec, 10, args, config(), shared)
            prefix = f'c.decode.step{rec.step:02d}.boundary.10'
            meta = rec.boundaries[prefix]
            self.assertEqual(meta['kv_rows'], (start + tokens) // 2)
            self.assertEqual(meta['previous_kv_rows'], start // 2)
            torch.testing.assert_close(rec.tensors[prefix + '.residual'], args[0][0].float())
            torch.testing.assert_close(rec.tensors[prefix + '.pre_mix'], args[2][0])
            torch.testing.assert_close(rec.tensors[prefix + '.kv'],
                                       shared.compress_kv[0, :(start + tokens) // 2])
            self.assertEqual(rec.tensors[prefix + '.selection'].tolist(), [[0, 1, -1, -1]] * tokens)
            self.assertEqual(rec.tensors[prefix + '.index_keys'].shape[0], (start + tokens) // 2)
            for k, v in vars(shared).items():
                torch.testing.assert_close(v, before[k])
            # Copies remain a snapshot if the reference later reuses its cache.
            saved = rec.tensors[prefix + '.kv'].clone()
            shared.compress_kv.zero_()
            torch.testing.assert_close(rec.tensors[prefix + '.kv'], saved)

    def test_later_stage_carries_candidate_bits_but_no_unused_index_keys(self):
        rec, args, shared = self.inputs(1001, 1)
        shared.candidates[..., 2:4] = False
        dump.capture_boundary(rec, 30, args, config(), shared)
        prefix = 'c.decode.step01.boundary.30'
        self.assertEqual(rec.tensors[prefix + '.candidate_mask'].tolist(), [[1, 0, 1, 1]])
        self.assertNotIn(prefix + '.index_keys', rec.tensors)
        self.assertEqual(rec.boundaries[prefix]['kv_rows'], 1002)
        dump.capture_boundary(rec, 20, args, config(), shared)
        self.assertEqual(set(k.rsplit('.', 1)[-1] for k in rec.tensors if '.boundary.20.' in k),
                         {'residual', 'pre_mix'})

    def test_untracked_and_other_rank_calls_capture_nothing(self):
        for rank, tracking, layer in [(1, True, 10), (0, False, 10), (0, True, 11)]:
            rec, args, shared = self.inputs(0, 17)
            rec.rank, rec.tracking = rank, tracking
            dump.capture_boundary(rec, layer, args, config(), shared)
            self.assertFalse(rec.tensors)
            self.assertFalse(rec.boundaries)


if __name__ == '__main__':
    unittest.main()
