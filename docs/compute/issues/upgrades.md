Agent Upgrades — Spec & Issues (v0.1)

Status: Draft 0.1
Scope: Declarative, policy‑compliant “Agent Upgrades” that agents can discover, schedule (e.g., overnight), install, and run on macOS workers only; iOS acts as coordinator/UI (no code execution, no background compute).
Ties to: OpenAgents Compute Marketplace Phases 1–3
Apple Compliance: iOS is coordination‑only; execution and scheduling occur on macOS. No downloaded code on iOS. Foundation Models AUP enforced. (See docs/compute/apple-terms-research.md.)

⸻

1) Goals & Non‑Goals

Goals
	•	A cross‑platform, declarative format (JSON manifest) for upgrades: what it does, when to run (cron/window), required backends (Foundation Models, MLX, Ollama, llama.cpp), resource bounds, permissions, pricing, licensing, and policy constraints.
	•	Semi‑public marketplace where upgrades are discoverable (via Nostr) and installable with explicit user consent, with author reputation and payout flows.
	•	Scheduling primitives for “overnight” and other windows, with system‑friendly constraints (charging, Wi‑Fi, idle, Do Not Disturb), jitter, catch‑up policy.
	•	Payments & licensing (Spark SDK + Nostr receipts) and reputation (ratings/attestations) without central servers.

Non‑Goals (v0.1)
	•	No arbitrary code execution on iOS; no long‑running iOS background work.
	•	No in‑app code download/exec on iOS (ASRG 2.5.2).
	•	macOS code‑package upgrades are allowed but optional; default is declarative JSON pipelines.

⸻

2) Upgrade Types
	1.	Declarative Upgrade (JSON only) — portable and safest. Defines inputs, steps (model invocation/tool‑call/HTTP), outputs, and schedule; runs inside worker’s built‑in operators; no external code. Works everywhere; compliant with iOS coordination model.
	2.	Code‑Package Upgrade (macOS‑only) — optional. A signed bundle (WASI module or notarized macOS plugin) referenced by the manifest. Download/exec occurs only on macOS worker with sandboxing, signature checks, and explicit permissions. Never on iOS.

v0.1 focuses on Declarative. Code‑Package support is designed, but behind a feature flag until Phase 3.

⸻

3) JSON Manifest (“Upgrade Manifest”)

Top‑level fields

{
  "$schema": "https://openagents.dev/schemas/upgrade-manifest-v0.1.json",
  "id": "openagents.upgrade.nightly-index",
  "version": "0.1.3",
  "title": "Nightly Code Index Refresh",
  "summary": "Refresh embeddings and symbol graph from your repo while you sleep.",
  "author": {
    "name": "OpenAgents",
    "npub": "npub1...",
    "contact": "hello@openagents.dev"
  },
  "license": "Apache-2.0",
  "homepage": "https://openagents.dev/upgrades/nightly-index",
  "categories": ["developer-tools", "search"],
  "tags": ["searchkit", "embeddings", "cron"],

  "capabilities": {
    "platforms": ["macos"],
    "backends": ["foundation_models", "mlx"],
    "requires": {
      "disk_mb": 2048,
      "ram_mb": 4096,
      "gpu": false
    }
  },

  "permissions": {
    "filesystem": ["~/Projects/**", "~/Library/Application Support/OpenAgents/indices/**"],
    "network": ["github.com", "api.openagents.dev"],
    "tools": ["searchkit.index", "git.clone", "model.embed"]
  },

  "schedule": {
    "type": "cron",
    "expression": "0 2 * * *",
    "timezone": "America/Chicago",
    "window": { "start": "01:00", "end": "05:00" },
    "constraints": {
      "plugged_in": true,
      "wifi_only": true,
      "cpu_max_percentage": 60,
      "suspend_if_active": true,
      "respect_dnd": true
    },
    "jitter_ms": 600000,
    "on_missed": "catch_up"
  },

  "triggers": [
    { "type": "file_change", "glob": "~/Projects/**.swift" },
    { "type": "nostr_event", "kinds": [1], "filter": {"#t": ["reindex"]} }
  ],

  "pipeline": [
    { "op": "git.clone", "repo": "https://github.com/user/repo", "dest": "~/Projects/repo", "update": true },
    { "op": "searchkit.index", "path": "~/Projects/repo", "index": "dev-repo", "languages": ["swift", "ts"], "include": ["src/**"], "exclude": ["node_modules/**"] },
    { "op": "model.embed", "backend": "mlx", "index": "dev-repo", "dim": 1536 }
  ],

  "pricing": { "model": "per-run", "amount_sats": 50, "revenue_split": [{"npub": "npub1...author", "bps": 8000}, {"npub": "npub1...curator", "bps": 2000}] },

  "policy": {
    "aup_flags": ["no_guardrail_bypass", "no_regulated_health", "no_legal_advice"],
    "data_retention_days": 14,
    "telemetry_level": "basic"
  },

  "artifacts": [
    { "type": "doc", "url": "https://cdn.openagents.dev/upgrades/nightly-index/readme.html", "sha256": "..." }
  ],

  "signing": {
    "manifest_sha256": "...",
    "nostr_event_id": "...",
    "sig_author": "..."
  }
}

JSON Schema (abridged)

{
  "$id": "https://openagents.dev/schemas/upgrade-manifest-v0.1.json",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "required": ["id", "version", "title", "author", "capabilities", "schedule", "pipeline"],
  "properties": {
    "id": {"type": "string", "pattern": "^[a-z0-9_.-]+$"},
    "version": {"type": "string", "pattern": "^\n?\n?\n?\n?\n?\n?\n?\n?\n?\n?\n?\n?\n$"},
    "title": {"type": "string", "minLength": 3},
    "author": {
      "type": "object",
      "required": ["npub"],
      "properties": {"name": {"type": "string"}, "npub": {"type": "string"}, "contact": {"type": "string"}}
    },
    "capabilities": {
      "type": "object",
      "required": ["platforms", "backends"],
      "properties": {
        "platforms": {"type": "array", "items": {"enum": ["macos", "linux", "windows"]}},
        "backends": {"type": "array", "items": {"enum": ["foundation_models", "mlx", "ollama", "llama.cpp"]}},
        "requires": {
          "type": "object",
          "properties": {
            "disk_mb": {"type": "integer", "minimum": 0},
            "ram_mb": {"type": "integer", "minimum": 0},
            "gpu": {"type": "boolean"}
          }
        }
      }
    },
    "permissions": {
      "type": "object",
      "properties": {
        "filesystem": {"type": "array", "items": {"type": "string"}},
        "network": {"type": "array", "items": {"type": "string"}},
        "tools": {"type": "array", "items": {"type": "string"}}
      }
    },
    "schedule": {
      "type": "object",
      "required": ["type"],
      "properties": {
        "type": {"enum": ["cron", "manual", "event"]},
        "expression": {"type": "string"},
        "timezone": {"type": "string"},
        "window": {"type": "object", "properties": {"start": {"type": "string"}, "end": {"type": "string"}}},
        "constraints": {
          "type": "object",
          "properties": {
            "plugged_in": {"type": "boolean"},
            "wifi_only": {"type": "boolean"},
            "cpu_max_percentage": {"type": "integer", "minimum": 1, "maximum": 100},
            "suspend_if_active": {"type": "boolean"},
            "respect_dnd": {"type": "boolean"}
          }
        },
        "jitter_ms": {"type": "integer", "minimum": 0},
        "on_missed": {"enum": ["skip", "catch_up"]}
      }
    },
    "triggers": {"type": "array", "items": {"type": "object"}},
    "pipeline": {"type": "array", "minItems": 1},
    "pricing": {"type": "object"},
    "policy": {"type": "object"},
    "artifacts": {"type": "array"},
    "signing": {"type": "object"}
  }
}

iOS compliance: iOS may display, install, and configure upgrades (UI & metadata), but never executes or downloads code. Scheduling and execution occur on macOS worker.

⸻

4) Scheduling Semantics
	•	Cron: standard 5‑field m h dom mon dow with TZ awareness.
	•	Window: optional local quiet‑hours window (e.g., 01:00–05:00).
	•	Constraints: plugged_in, wifi_only, cpu_max_percentage, respect_dnd, suspend_if_active (pause if significant user input).
	•	Jitter: random delay to avoid “thundering herd”.
	•	Catch‑up: if missed (sleep/offline), run once at next availability or skip.

Adapters
	•	macOS: translate to launchd or internal scheduler; detect power/idle via IOKit; pause/resume jobs.
	•	Linux: systemd timers optional; default to internal scheduler.
	•	Windows: Task Scheduler optional; default to internal scheduler.

⸻

5) Nostr Mapping (Discovery • Reputation • Licensing)

Event kinds (app‑specific using NIP‑33 parameterized replaceable)
	•	kind 30051 — oa.upgrade.manifest
	•	d tag: upgrade slug (e.g., openagents.upgrade.nightly-index)
	•	Content: the manifest JSON
	•	Other tags: v (version), c (categories), t (tags), b (backends), l (license)
	•	kind 30052 — oa.upgrade.release
	•	d: slug@version
	•	Content: release notes + artifact checksums/URLs (for docs or macOS‑only code packages)
	•	kind 30053 — oa.upgrade.license.receipt
	•	One per buyer per version (or durable entitlement)
	•	Tags: a (points to 30051), p (author npub), bolt11, spark_receipt_id, optional zap proof (NIP‑57), revshare tuples
	•	kind 30054 — oa.upgrade.reputation
	•	Parameterized by upgrade slug and rater npub; includes rating (1–5), short review, and optional attestation refs to completed job IDs
	•	Curated lists: use NIP‑51 Lists (kind 30001) with l:upgrades to publish “Top Upgrades”, “Trusted Authors”, etc.

Discovery flow
	1.	Client queries relays for 30051 (manifests) + 30052 (latest releases).
	2.	Filter by backend/price/tags; sort by reputation aggregates (from 30054) and curator lists (NIP‑51).
	3.	For paid upgrades, validate presence of 30053 license receipt for buyer.

⸻

6) Payments & Licensing (Spark + Nostr)
	•	Pricing models: free, per-run, per-month.
	•	Buyer pays via Spark SDK (BOLT11 compatible).
	•	Payment coordinator posts 30053 license.receipt with payment proof; macOS worker validates before running paid steps.
	•	Revenue split supported via receipt tags (basis points per npub).
	•	Optional NIP‑57 zap mirror for public tip signal.

If results are consumed in iOS, we support IAP in that flow (ASRG 3.1.1). For most upgrades that produce files/indices used outside the app, payments occur outside IAP (Spark/Apple Pay), consistent with “out‑of‑app consumption”.

⸻

7) Reputation
	•	Signals: 30054 rating events, NIP‑57 zaps, install counts (optional anonymized aggregates), job success/failure rates (attested by workers).
	•	Aggregation: parameterized replaceable “roll‑up” events per upgrade maintained by neutral indexers (or locally by client).

⸻

8) Security & Policy
	•	Consent UI: show manifest diff on install/update; enumerate permissions; require explicit approval.
	•	Policy engine: deny if manifest violates Foundation Models AUP or local policy (e.g., disallow network egress).
	•	Resource guard: enforce CPU/memory caps; auto‑pause under user activity.
	•	Supply chain: signed manifests, checksums; code packages require signature + notarization (macOS).
	•	Privacy: clear data retention and export; local‑only option.

⸻

9) Component Changes
	•	iOS (Coordinator):
	•	Upgrade catalog (list/search/filter).
	•	Install/config flows; schedule editor.
	•	License purchase (Spark/IAP depending on consumption) and seat management.
	•	Reputation UI (ratings, zaps), curator lists.
	•	macOS (Worker):
	•	Manifest runner (declarative ops: model/tool/http/fs/index).
	•	Scheduler adapter; constraints monitor; pause/resume.
	•	Payment receipt verifier; policy engine; telemetry exporter.
	•	Optional code‑package loader (behind flag).

⸻

10) E2E Example Flow (Nightly Index)
	1.	User installs Nightly Code Index Refresh from iOS catalog; sets window 01:00–05:00, plugged_in=true.
	2.	iOS sends signed install config to macOS worker.
	3.	At 02:00 while charging on Wi‑Fi, worker runs pipeline → updates index → posts completion to Nostr (kind 1 note referencing upgrade id).
	4.	Buyer licenses a paid “Code Linter Pro” upgrade; Spark payment succeeds; 30053 receipt appears; worker begins running on next schedule.

⸻

11) GitHub Issues (New)

032 – Upgrade Manifest Spec (JSON Schema + Examples)
P0 · 1–2w
Acceptance: schema file, generators, and 3 working examples validated in CI.

033 – Nostr Event Mapping for Upgrades (kinds 30051–30054) + Indexer
P0 · 1–2w
Acceptance: publish/subscribe helpers; queries by tags; basic roll‑up aggregator.

034 – macOS Scheduler Adapter (launchd + internal)
P0 · 1–2w
Acceptance: cron+window+constraints; pause/resume; missed‑run policy; tests.

035 – iOS UI: Upgrade Catalog & Install Flow
P0 · 2w
Acceptance: browse→details→install; manifest diff; permissions screen; no code exec.

036 – macOS Declarative Runner (Ops Engine v1)
P0 · 3–4w
Acceptance: built‑ins: git.clone, searchkit.index, model.embed, http.request, fs.write; telemetry + logs.

037 – Reputation Events + Aggregation
P1 · 1–2w
Acceptance: rating post; aggregator computes score + trend; UI badges.

038 – Payments: License Receipts (Spark + Nostr 30053)
P0 · 2w
Acceptance: pay → receipt emitted → worker verifies before run.

039 – Upgrade Authoring CLI
P1 · 1–2w
Acceptance: oa upgrade init/validate/publish; schema validation; checksum; sign.

040 – Security: Signing, Permissions & Sandboxing
P0 · 2w
Acceptance: manifest signing; permissions enforcement; macOS notarization checks.

041 – Versioning & Rollback
P1 · 1w
Acceptance: semver, channel tags, safe rollback on failure.

042 – Compliance Gate (AUP/iOS Rules)
P0 · 1w
Acceptance: deny lists; preflight checks; iOS “coordination‑only” assertions.

043 – Moderation & Takedowns
P2 · 1w
Acceptance: blocklist feeds; curator overrides; UI surfacing.

044 – Marketplace Web Mirror (Static)
P2 · 1w
Acceptance: static site mirrors Nostr upgrade listings for SEO; read‑only.

045 – Capability Negotiation & Compatibility Matrix
P1 · 1w
Acceptance: match upgrade requirements to worker capabilities; explain “why not runnable”.

046 – Cron Semantics & Sleep Handling
P1 · 1w
Acceptance: unit tests for edge cases (DST, leap, missed runs, jitter bounds).

047 – Telemetry & Resource Guard
P1 · 1–2w
Acceptance: CPU/mem caps; user activity pause; logs + Prometheus exporter.

048 – Offline License Verification
P2 · 3–5d
Acceptance: cached receipt proofs; grace periods; replay protection.

⸻

12) Testing Strategy
	•	Schema tests: JSON fixtures; CI validation; golden examples.
	•	Scheduler tests: DST transitions; missed‑run recovery; jitter distribution.
	•	Runner tests: mocked tools/backends; resource caps; cancellation.
	•	Payments tests: fake Spark node; receipt creation/verification; failure paths.
	•	Reputation tests: spam/abuse filters; aggregation integrity.
	•	Compliance tests: policy engine denies prohibited manifests (AUP), iOS asserts coordination‑only.

⸻

13) Open Questions (Track in ADRs)
	•	Standardize on NIP numbers for app‑specific kinds vs. reuse existing (e.g., NIP‑94/96 for file metadata/uploads)?
	•	Trust model for curator lists; sybil resistance for ratings.
	•	Optional “trial mode” for paid upgrades (metered runs) without receipts.

⸻

14) Roadmap Fit
	•	Phase 1: 032–036, 042, 046.
	•	Phase 2: 038, 048, payments wiring in catalog/worker.
	•	Phase 3: 037, 039–041, 043–045, 047.