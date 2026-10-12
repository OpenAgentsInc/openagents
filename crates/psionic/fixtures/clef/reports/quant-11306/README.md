# Clef 27B quantization against routes-v5 and the chat goldens (#11306)

Status: **running**. The frozen plan and the scripts are here; the results
are not in yet. Nothing in this directory changes a default: Jev stays first
until a candidate passes the plan's product gates.

## What this step evaluates

The issue's first artifact step: does a larger, standard quantization that
fits the 52 GiB execution cap trade quality against latency usefully? The
candidate is `Clef-Q8_0.gguf` from `ggml-org/Clef-GGUF` (28,732,215,360
bytes, 26.76 GiB). It comes from the same upstream conversion as the
released `Clef-Q4_K_M.gguf` (`.src_sha` `PRIMARY=0b331204bb13fbd2ca93a64df1956f5b55478ce5`),
with the same joint head, so the checkpoint and head identities are
kept. Psionic's CUDA and Metal Clef lanes already dequantize Q8_0
(`ClefWeightFormat::Q8_0`), so no runtime change is needed.

The fixed FFN-down Q6 candidate (`fixed-down-q6-v1`, PR #11299) is kept but
not run here: it was generated on the Mac and is not published anywhere this
VM can fetch it.

## The frozen plan

[`plan.json`](plan.json) fixes everything a comparison must share:

- **Corpora, with hashes.** routes-v5 (1,073 rows: 799 `tune`, 274
  `held_out`) and chat-goldens `goldens-v1.json` (123 cases, gate
  `min_right` 0.9 with no wrong case in a critical flow).
- **Splits.** routes-v5 `tune` is the only data a map is fitted on;
  routes-v5 `held_out` and all chat goldens are evaluation only. Nothing is
  selected or fitted on held-out errors.
- **Conditions.** One router revision (`dcf31b1e8b`, staged routing as
  shipped), one answer bank, product notes on for every row (Jev too, unlike
  the 2026-10-10 H100 run), a freshly started server per Clef row so the
  exact logit cache is empty, and the same psionic flags.
- **Systems.** Hosted Jev; 27B Q4 and Q8_0, each with the inherited
  Clef-Flash staged map and raw; and for each artifact a 27B map fitted on
  routes-v5 `tune` and scored on `held_out` raw and calibrated
  (`ROUTER_EVAL_PUBLISH=1`, issue step 5).

## Why an H100 and not the 4080

The 27B Q4 holds 18.1 GB of VRAM after load in psionic's CUDA lane (H100
measurement, 2026-10-10). The RTX 4080 on `coderos-4080` has 16 GB, about
10 GB of it already held by the running Clef-Flash servers, so 27B cannot run
there; Q8_0 needs more still. The run uses a GCE `a3-highgpu-1g` spot VM
(H100 80 GB, `us-east5-a`), deleted after the run.

## Running it

1. `vm-startup.sh` (the VM's startup script) builds `psionic-openai-server`
   at the plan's revision, downloads both GGUFs, checks their sizes, records
   their SHA-256 and the binary's, and writes `serve.sh q4|q8`.
2. `run-goldens.sh all` builds `chat-goldens` and the routes-v5 eval at the
   same revision, then runs, in order: Jev; Q4 with map, Q4 raw, Q4 fit; Q8
   with map, Q8 raw, Q8 fit. Keys reach the VM in a root-only file that the
   runner reads and deletes; nothing writes them elsewhere.

Read progress and results on the VM:

```sh
export CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config
gcloud compute ssh clef-11306-h100 --zone us-east5-a --project openagentsgemini \
  --command 'cat /var/log/clef-11306/state /var/log/clef-11306/run-state; ls /var/log/clef-11306/results'
gcloud compute scp --recurse --zone us-east5-a --project openagentsgemini \
  clef-11306-h100:/var/log/clef-11306 ./crates/psionic/fixtures/clef/reports/quant-11306/vm-logs
```

Each `results/<system>-<map>/chat-goldens/report.json` holds route, tier,
answer, critical-flow and latency numbers; `results/<q>-fit/` holds the
routes-v5 evidence record and the fitted `calibration-v2.json`.

## Decision rule

Jev stays first unless a candidate meets the 2 s judge budget and the
accuracy gates, critical errors included. A smaller BF16 logit difference
alone is not success. Failed candidates keep their results here.
