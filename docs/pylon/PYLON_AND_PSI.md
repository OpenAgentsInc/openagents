# Pylon and Psionic: How They Relate

Status: summary
Date: 2026-03-07

This document summarizes how **Pylon** (the standalone provider connector) and **Psionic** (the in-repo inference/embeddings runtime) fit together for the OpenAgents Compute Market.

## In very simple language

**What Pylon does**  
Pylon is the program you run on your computer so that your computer can **sell** its AI capacity (text generation, embeddings) to the OpenAgents network. You install Pylon, turn it on, and it tells the network “this machine can do X.” When someone buys that capacity, Pylon runs the job on your machine and reports back. So: **Pylon = “put my machine on the market.”**

**What Psionic does**  
Psionic is the **engine** that actually runs the AI model on your machine. It loads the model file, does the math (inference or embeddings), and returns the result. Today we use Ollama for that; Psionic will replace Ollama so everything stays in our own code and we can prove exactly what ran. So: **Psionic = “the thing that runs the model.”**

**Together**  
You run Pylon so your machine joins the market. When a job comes in, Pylon asks something on your machine to run the model — today that’s Ollama, later it’ll be Psionic. Pylon is the connector; Psionic is the runner.

## One-sentence version

**Pylon runs on any machine and exposes that machine’s compute to the market; Psionic is the execution engine that will perform inference and embeddings when the provider uses it as the backend.**

## Roles

| Component | Role |
|-----------|------|
| **Pylon** | Standalone provider binary. Runs on a laptop, desktop, workstation, or server. Detects local backends, publishes inventory to the network, accepts jobs, executes them using those backends, emits delivery evidence and receipts, tracks payouts. **Connector** — turns a machine into network-visible supply. |
| **Psionic** | Standalone Rust runtime in `OpenAgentsInc/psionic`. Loads GGUF (and compatible) models, runs inference and embeddings on CPU/Metal/NVIDIA/AMD. **Engine** — does the actual tensor work when a job runs. |
| **Compute market** | Buyers request inference/embeddings (and later sandbox execution); Nexus is the authority; providers (Pylon or Autopilot embedding the same substrate) offer supply and execute jobs locally. |

## “Run on any computer and give access to certain resources”

- **Pylon** is the thing that “runs on any computer.” You install it, run `pylon init`, `pylon serve`, `pylon online`. It discovers what that machine can do (e.g. “Ollama is available,” or later “Psionic is available,” “Apple FM is available,” and eventually “sandbox profile X is available”).
- **Resources** = the compute products that machine can truthfully offer: e.g. `ollama.text_generation`, `ollama.embeddings`, `apple_foundation_models.text_generation`. Those become **inventory** published to the OpenAgents network. Buyers see and procure that supply; when a job is assigned to this provider, Pylon runs it locally using the appropriate backend.
- So: the computer runs Pylon → Pylon detects backends → Pylon advertises products → market gets access to that machine’s inference/embeddings (and later sandbox) capacity.

## “The compute market will use Psionic”

- Today, launch backends are **Ollama** (inference + embeddings) and **Apple Foundation Models** (inference). So today the market “uses” whatever backends the provider has — typically Ollama on the provider machine.
- **Psionic** is the planned replacement for the desktop’s (and eventually the provider’s) **Ollama dependency**. The Psionic roadmap (`https://github.com/OpenAgentsInc/psionic/blob/main/docs/ROADMAP.md`) is to:
  - Replace Ollama with an in-process Rust runtime (Psionic) that can load GGUF, serve inference and embeddings, and report truthful capability and evidence.
  - Make Psionic the **reusable execution substrate** for the compute market: same engine can back both the Autopilot desktop and a headless Pylon node.
- So “the compute market will use Psionic” means: once Psionic is ready and cut over, the **same** provider substrate (used by Pylon and by Autopilot) will call **Psionic** instead of (or in addition to) Ollama to execute inference and embeddings jobs. Product names may shift from `ollama.text_generation` / `ollama.embeddings` to backend-neutral or `psionic.*`; the important part is that the **engine** doing the work is Psionic, with truthful capability envelopes, delivery proofs, and settlement linkage (PSI-171 through PSI-175 in the Psionic roadmap).

## Dependency flow

1. **Provider substrate** (`#3116`, then extracted to something like `crates/openagents-provider-substrate`): canonical backend detection, product derivation, lifecycle, receipts, evidence. This is **backend-agnostic** — it talks to “whatever runs inference/embeddings here” (today Ollama, tomorrow Psionic).
2. **Pylon** = standalone binary on top of that substrate. No direct dependency on Psionic; it depends on “some backend(s) that implement the execution contract.”
3. **Psionic** = one possible backend. When the desktop (or a Pylon node) uses Psionic as the local runtime, Psionic is what actually runs the model and returns tokens/embeddings. Psionic must therefore expose the **same contract** the provider substrate expects (inference/embeddings API, capability reporting, delivery evidence).
4. **Cutover**: Autopilot switches from Ollama HTTP to in-process Psionic (OA-201, OA-202). After that, a machine can run “Pylon + Psionic” (or “Autopilot embedding substrate + Psionic”) and the compute market will be using Psionic for that node’s inference/embeddings supply.

## Summary diagram (conceptual)

```
[Any machine]
     │
     ├── Pylon (or Autopilot embedding same substrate)
     │        │
     │        ├── Detects backends
     │        ├── Publishes inventory → OpenAgents network (compute market)
     │        ├── Accepts jobs from market
     │        └── Executes jobs by calling local backend(s)
     │
     └── Backends (what actually runs the work)
              ├── Today: Ollama (inference, embeddings), Apple FM (inference)
              └── After cutover: Psionic (inference, embeddings), Apple FM (inference), …
```

So: **Pylon = connector that runs anywhere and gives the market access to this machine’s resources; Psionic = the engine the market will use for inference and embeddings once it replaces Ollama as the standard in-repo runtime.**

## References

- **Pylon**: `docs/pylon/PYLON_PLAN.md`, `docs/pylon/README.md`
- **Psionic**: `https://github.com/OpenAgentsInc/psionic/blob/main/docs/ROADMAP.md` (objective, Epic F compute-market substrate, definition of done for “Replace Ollama” and “Psionic as compute-market substrate”)
- **Compute market program**: issue `#3116`, launch products and capability envelope in PYLON_PLAN.md
