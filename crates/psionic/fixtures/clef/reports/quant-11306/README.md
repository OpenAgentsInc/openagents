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
(H100 80 GB). The project has no on-demand H100 quota (`GPUS_PER_GPU_FAMILY`
is 0 in every region tried), so spot is the only option.

## History

- 2026-10-12 04:30 UTC: first VM (`us-east5-a`, spot, deleted on termination)
  was preempted at 05:31 before any row finished. Its results lived only on
  its disk and were lost.
- 2026-10-12 10:40 UTC: relaunched in `us-central1-a`, spot, **stopped** (not
  deleted) on preemption, 10-hour cap. The run is now unattended and
  resumable, and each finished row is copied to GCS.

## Running it

1. `vm-startup.sh` (the VM's startup script, run on every boot) builds
   `psionic-openai-server` at the plan's revision, downloads both GGUFs,
   checks their sizes, records their SHA-256 and the binary's, and writes
   `serve.sh q4|q8`. When the instance has `oa-autorun=1` and carries
   `run-goldens.sh` as the `oa-runner` metadata value, it then starts
   `run-goldens.sh all` in the background.
2. `run-goldens.sh all` builds `chat-goldens` and the routes-v5 eval at the
   same revision, then runs, in order: Jev; Q4 with map, Q4 raw, Q4 fit; Q8
   with map, Q8 raw, Q8 fit. A finished row writes `results/<row>/done` and is
   skipped on a rerun. Keys are read from Secret Manager through the VM's
   service account (`oa-decision-bench`: `decision-throughput-typesafe-key`
   for Jev, `coder-ai-gateway-key` for product notes) and never written to
   disk.
3. After every row, and every 10 minutes, `/var/log/clef-11306` (logs and
   results, not models or build trees) is copied to
   `gs://openagentsgemini-decision-throughput/clef-11306/clef-11306-h100/`.

Launch (as the automation account):

```sh
export CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config
cd crates/psionic/fixtures/clef/reports/quant-11306
gcloud compute instances create clef-11306-h100 --project openagentsgemini --zone us-central1-a \
  --machine-type a3-highgpu-1g --provisioning-model SPOT --max-run-duration=10h \
  --maintenance-policy TERMINATE --instance-termination-action STOP \
  --discard-local-ssds-at-termination-timestamp=true \
  --image-family common-cu129-ubuntu-2204-nvidia-580 --image-project deeplearning-platform-release \
  --boot-disk-size 400GB --boot-disk-type pd-ssd \
  --service-account oa-decision-bench@openagentsgemini.iam.gserviceaccount.com --scopes cloud-platform \
  --metadata oa-rev=dcf31b1e8bfbdc5d867c1c1325715eac3f6589cd,oa-autorun=1,install-nvidia-driver=True \
  --metadata-from-file startup-script=vm-startup.sh,oa-runner=run-goldens.sh
```

Check progress, resume after a preemption, and collect results:

```sh
export CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config
gcloud compute instances describe clef-11306-h100 --zone us-central1-a --project openagentsgemini --format='value(status)'
gcloud storage cat gs://openagentsgemini-decision-throughput/clef-11306/clef-11306-h100/progress.log
# TERMINATED means preempted or out of time: start it again, it resumes at the next unfinished row
gcloud compute instances start clef-11306-h100 --zone us-central1-a --project openagentsgemini
# results into this directory
gcloud storage rsync -r gs://openagentsgemini-decision-throughput/clef-11306/clef-11306-h100 ./vm-logs
# when run-state says done: delete the VM (its 400 GB disk costs money while stopped)
gcloud compute instances delete clef-11306-h100 --zone us-central1-a --project openagentsgemini -q
```

Each `results/<system>-<map>/chat-goldens/report.json` holds route, tier,
answer, critical-flow and latency numbers; `results/<q>-fit/` holds the
routes-v5 evidence record and the fitted `calibration-v2.json`.

## Decision rule

Jev stays first unless a candidate meets the 2 s judge budget and the
accuracy gates, critical errors included. A smaller BF16 logit difference
alone is not success. Failed candidates keep their results here.
