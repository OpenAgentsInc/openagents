# Kimi K3 on Google Cloud — self-hosting evaluation

This folder tracks the evaluation of self-hosting Moonshot AI's **Kimi K3**
(2.8T-parameter MoE model) on our Google Cloud project, prompted by Google's
Day-0 support announcement (2026-07-30).

The driving question: **can our Google Cloud credits apply toward hosting
Kimi K3 ourselves, and if so, which of Google's recommended deployment paths
makes the most sense for OpenAgents?**

## Contents

- [`2026-07-30-google-cloud-day0-announcement.md`](2026-07-30-google-cloud-day0-announcement.md)
  — digest of the Google announcement: the three deployment paths, the
  multi-node SGLang/DSPARK architecture, the full GKE deployment recipe, and
  every external link from the post.
- [`2026-07-30-gcloud-credits-investigation.md`](2026-07-30-gcloud-credits-investigation.md)
  — live investigation of our project (`openagentsgemini`): billing/credit
  state, quota reality for the required GPU families, and which path is
  actually available to us.

## The three deployment paths (summary)

| Path | What it is | Best for |
| --- | --- | --- |
| **Model Garden** (Gemini Enterprise Agent Platform) | One-click deploy to a managed, production-grade endpoint | Fastest path; billed as Vertex/agent-platform usage |
| **AI Hypercomputer recipes** | Validated GKE + SGLang configurations on GitHub | Teams running their own orchestration on reserved GPU capacity |
| **GKE + llm-d** | Kubernetes-native distributed serving (KV-aware routing, disaggregated prefill/decode) | Massive scale, deep customization |

## Hard constraints to keep in mind

- The model needs **16 GPUs minimum** (TP=16): 2× A4 nodes (8× B200 each) or
  4× A4X nodes (GB200 NVL72). This is top-of-line accelerator capacity with
  scarce quota and, typically, reservation-based availability.
- Kimi K3 ships under its own license
  ([LICENSE](https://huggingface.co/moonshotai/Kimi-K3/blob/main/LICENSE));
  review before any deployment.
- Whether credits apply depends on the SKU family the chosen path bills
  against (Vertex AI / Model Garden endpoint pricing vs. GCE/GKE accelerator
  VM pricing) and on any credit-program exclusions. See the investigation doc
  for what our project actually shows.

## Source

- Announcement: <https://discuss.google.dev/t/announcing-day-0-support-for-kimi-k3-on-google-cloud/385392>
