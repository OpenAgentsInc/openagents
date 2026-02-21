# This file is responsible for configuring your application
# and its dependencies with the aid of the Config module.
#
# This configuration file is loaded before any dependency and
# is restricted to this project.

# General application configuration
import Config

config :openagents_runtime,
  namespace: OpenAgentsRuntime,
  ecto_repos: [OpenAgentsRuntime.Repo],
  generators: [timestamp_type: :utc_datetime],
  guarded_outbound_http_enabled: true,
  khala_projection_sink: OpenAgentsRuntime.Khala.NoopSink,
  khala_projection_version: "khala_summary_v1",
  janitor_scan_interval_ms: 5_000,
  janitor_max_recovery_attempts: 3,
  janitor_recovery_cooldown_ms: 30_000,
  l1_compaction_interval_ms: 60_000,
  l1_compaction_batch_size: 3,
  l1_compaction_min_events: 30,
  khala_sync_retention_interval_ms: 60_000,
  khala_sync_retention_horizon_seconds: 86_400,
  khala_sync_retention_batch_size: 5_000,
  khala_sync_topic_policies: %{
    "runtime.run_summaries" => %{
      topic_class: "durable_summary",
      retention_seconds: 604_800,
      compaction_mode: "tail_prune_with_snapshot_rehydrate",
      snapshot: %{
        enabled: true,
        format: "openagents.sync.snapshot.v1",
        schema_version: 1,
        cadence_seconds: 300,
        source_table: "runtime.sync_run_summaries"
      }
    },
    "runtime.codex_worker_summaries" => %{
      topic_class: "durable_summary",
      retention_seconds: 259_200,
      compaction_mode: "tail_prune_with_snapshot_rehydrate",
      snapshot: %{
        enabled: true,
        format: "openagents.sync.snapshot.v1",
        schema_version: 1,
        cadence_seconds: 120,
        source_table: "runtime.sync_codex_worker_summaries"
      }
    },
    "runtime.codex_worker_events" => %{
      topic_class: "high_churn_events",
      retention_seconds: 86_400,
      compaction_mode: "tail_prune_without_snapshot",
      snapshot: %{enabled: false}
    },
    "runtime.notifications" => %{
      topic_class: "ephemeral_notifications",
      retention_seconds: 43_200,
      compaction_mode: "tail_prune_without_snapshot",
      snapshot: %{enabled: false}
    }
  },
  khala_sync_replay_batch_size: 200,
  khala_sync_heartbeat_interval_ms: 15_000,
  khala_sync_heartbeat_timeout_ms: 60_000,
  khala_sync_parity_enabled: false,
  khala_sync_parity_interval_ms: 30_000,
  khala_sync_parity_sample_size: 200,
  khala_sync_parity_projection_names: ["run_summary", "codex_worker_summary"]

config :openagents_runtime, :laravel_internal,
  base_url: "",
  secret_fetch_path: "/api/internal/runtime/integrations/secrets/fetch",
  shared_secret: "",
  key_id: "runtime-internal-v1",
  signature_ttl_seconds: 60,
  request_timeout_ms: 2_500,
  default_secret_cache_ttl_ms: 60_000

config :openagents_runtime, :khala_http_sink,
  base_url: "",
  admin_key: "",
  run_summary_mutation_path: "runtime:upsertRunSummary",
  codex_worker_summary_mutation_path: "runtime:upsertCodexWorkerSummary",
  request_timeout_ms: 2_500

config :openagents_runtime, :khala_fanout_sink,
  sinks: [OpenAgentsRuntime.Khala.NoopSink, OpenAgentsRuntime.Sync.ProjectorSink]

config :openagents_runtime, :khala_sync, stream_payload_mode: :inline

config :openagents_runtime, :khala_sync_auth,
  issuer: "https://openagents.test",
  audience: "openagents-sync",
  claims_version: "oa_sync_claims_v1",
  allowed_algs: ["HS256"],
  compat_enforced: false,
  compat_protocol_version: "khala.ws.v1",
  compat_min_client_build_id: "00000000T000000Z",
  compat_max_client_build_id: nil,
  compat_min_schema_version: 1,
  compat_max_schema_version: 1,
  hs256_keys: %{
    "sync-auth-v1" => "dev-sync-signing-key"
  }

# Configures the endpoint
config :openagents_runtime, OpenAgentsRuntimeWeb.Endpoint,
  url: [host: "localhost"],
  adapter: Bandit.PhoenixAdapter,
  render_errors: [
    formats: [json: OpenAgentsRuntimeWeb.ErrorJSON],
    layout: false
  ],
  pubsub_server: OpenAgentsRuntime.PubSub,
  live_view: [signing_salt: "jKdufZJV"]

# Configures Elixir's Logger
config :logger, :default_formatter,
  format: "$time $metadata[$level] $message\n",
  metadata: [:request_id]

# Use Jason for JSON parsing in Phoenix
config :phoenix, :json_library, Jason

# Import environment specific config. This must remain at the bottom
# of this file so it overrides the configuration defined above.
import_config "#{config_env()}.exs"
