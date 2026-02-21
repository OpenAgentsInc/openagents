import Config

# Configure your database
#
# The MIX_TEST_PARTITION environment variable can be used
# to provide built-in test partitioning in CI environment.
# Run `mix help test` for more information.
config :openagents_runtime, OpenAgentsRuntime.Repo,
  username: "postgres",
  password: "postgres",
  hostname: "localhost",
  database: "openagents_runtime_test#{System.get_env("MIX_TEST_PARTITION")}",
  pool: Ecto.Adapters.SQL.Sandbox,
  pool_size: System.schedulers_online() * 2

# We don't run a server during test. If one is required,
# you can enable the server option below.
config :openagents_runtime, OpenAgentsRuntimeWeb.Endpoint,
  http: [ip: {127, 0, 0, 1}, port: 4002],
  secret_key_base: "7kJumiRDb/bTD+w98RZun6M0WWyhmMdzv4n0RXZ6tajxJVw/ImGTD0sDy0LutGGL",
  server: false

# Print only warnings and errors during test
config :logger, level: :warning

# Initialize plugs at runtime for faster test compilation
config :phoenix, :plug_init_mode, :runtime

config :openagents_runtime, :runtime_signature_secret, "test-runtime-signature-secret"

config :openagents_runtime, :khala_sync_auth,
  issuer: "https://openagents.test",
  audience: "openagents-sync-test",
  claims_version: "oa_sync_claims_v1",
  allowed_algs: ["HS256"],
  hs256_keys: %{
    "sync-auth-test-v1" => "sync-test-signing-key"
  }

config :openagents_runtime, :laravel_internal,
  base_url: "http://laravel.test",
  secret_fetch_path: "/api/internal/runtime/integrations/secrets/fetch",
  shared_secret: "test-runtime-internal-shared-secret",
  key_id: "runtime-internal-v1",
  signature_ttl_seconds: 60,
  request_timeout_ms: 500,
  default_secret_cache_ttl_ms: 25

config :openagents_runtime, :agent_process_idle_shutdown_ms, 100
config :openagents_runtime, :janitor_scan_interval_ms, 60_000
config :openagents_runtime, :janitor_recovery_cooldown_ms, 0
config :openagents_runtime, :l1_compaction_interval_ms, 60_000
config :openagents_runtime, :l1_compaction_batch_size, 1
config :openagents_runtime, :l1_compaction_min_events, 2
