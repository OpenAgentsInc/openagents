# MPS head parameter-view probe

This frozen v2 snapshot extends v1 without changing it. The probe ran 16 calls through the official BF16 decision head, with no backbone loaded. Each call started with the same saved input bytes and BF16 weight values. Every input and head-weight byte check passed before and after execution.

Case 014 matched the original BF16 logits in all eight calls. Case 034 returned NaNs in four calls; its other four calls differed by up to 0.29296875 in logit values. Cloning token-index slices, nonzero-offset linear parameters, or both did not reliably correct the failure. The exact operator or runtime cause remains unknown. These results do not establish native 27B acceptance.

The 12.90-second experiment used a 4 GiB process guard, a 2 GiB MPS allocator cap, a 16 GiB startup margin, a 12 GiB available-memory floor, and a 120-second deadline. Peak accounted memory was 2,398,603,328 bytes. The owned process group stopped, with no memory rejection or cleanup error. These are correctness diagnostics on a shared machine, not model latency measurements.

Nonfinite values are explicit strings accompanied by `logits_finite: false`; they remain failures. No tolerance or acceptance rule changed. No tensor payloads or model weights are included.

The reproducer requires `CLEF_WORKTREE`, `CLEF_DIAGNOSTIC_SCRATCH`, and `CLEF_REFERENCE_PYTHON`, plus the retained reference checkpoint and saved head inputs. Run `guard_parameter_views.py head-probe` through the same bounded memory and GPU leases. Existing outputs deliberately refuse overwrite. `sources.json` records original and published script hashes; `manifest.json` covers every published file.
