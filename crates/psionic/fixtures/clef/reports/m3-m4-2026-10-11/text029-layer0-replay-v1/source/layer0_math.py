"""Small f64 recurrence checks for the two unchanged official Delta operators."""


def delta_fixtures(torch, modeling):
    cases = []
    with torch.inference_mode():
        for n in (1, 17, 65):
            for extreme in (False, True):
                # Two key heads repeat across four value heads, with signed dyadic inputs.
                raw = torch.arange(n * 2 * 4).reshape(1, n, 2, 4).float()
                q = ((raw % 11) - 5) / 32
                k = ((raw % 7) - 3) / 16
                if extreme:
                    q[:, 0] = 0
                    k[:, 0] = 0
                q = q.repeat_interleave(2, dim=2)
                k = k.repeat_interleave(2, dim=2)
                v = ((torch.arange(n * 4 * 3).reshape(1, n, 4, 3).float() % 17) - 8) / 32
                g = torch.full((1, n, 4), -0.125 if extreme else 0.0)
                if extreme:
                    g[:, ::7] = -80
                beta = torch.full((1, n, 4), 0.5)
                initial = (torch.arange(4 * 4 * 3).reshape(1, 4, 4, 3).float() - 24) / 4096
                q64 = q.double()
                k64 = k.double()
                q64 = q64 * torch.rsqrt((q64 * q64).sum(-1, keepdim=True) + 1e-6) / 2
                k64 = k64 * torch.rsqrt((k64 * k64).sum(-1, keepdim=True) + 1e-6)
                state = initial.double().clone()
                outputs = []
                for t in range(n):
                    state = state * g[:, t].double().exp()[..., None, None]
                    delta = (v[:, t].double() - (state * k64[:, t, :, :, None]).sum(-2)) * beta[:, t, :, None].double()
                    state = state + k64[:, t, :, :, None] * delta[..., None, :]
                    outputs.append((state * q64[:, t, :, :, None]).sum(-2))
                expected = torch.stack(outputs, dim=1)
                entries = []
                for name, op in (("chunk64", modeling.torch_chunk_gated_delta_rule),
                                 ("recurrent", modeling.torch_recurrent_gated_delta_rule)):
                    before = [x.clone() for x in (q, k, v, g, beta, initial)]
                    output, final = op(q, k, v, g=g, beta=beta, initial_state=initial,
                                       output_final_state=True, use_qk_l2norm_in_kernel=True)
                    if not all(torch.equal(x, y) for x, y in zip(before, (q, k, v, g, beta, initial))):
                        raise ValueError("fixture operator mutated inputs")
                    if not (bool(torch.isfinite(output).all()) and bool(torch.isfinite(final).all())
                            and torch.allclose(output.double(), expected, atol=1e-6, rtol=1e-5)
                            and torch.allclose(final.double(), state, atol=1e-6, rtol=1e-5)):
                        raise ValueError("official Delta operator failed independent f64 fixture: " + name)
                    entries.append({"operator": name, "output_max_abs": float((output.double() - expected).abs().max()),
                                    "state_max_abs": float((final.double() - state).abs().max())})
                cases.append({"tokens": n, "zero_qk_and_extreme_decay": extreme,
                              "nonzero_initial_state": True, "grouped_heads": True, "operators": entries})
    return {"passed": True, "atol": 1e-6, "rtol": 1e-5, "cases": cases,
            "scope": "Fixture screen only, not a full-model acceptance threshold or a bitwise Metal emulation."}
