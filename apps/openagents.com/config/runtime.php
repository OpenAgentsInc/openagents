<?php

return [
    /*
    |--------------------------------------------------------------------------
    | Runtime Driver
    |--------------------------------------------------------------------------
    |
    | Controls which runtime implementation handles chat runs.
    | - legacy: existing in-process Laravel RunOrchestrator
    | - elixir: external OpenAgents runtime service (internal HTTP/SSE)
    |
    */
    'driver' => env('OA_RUNTIME_DRIVER', 'legacy'),

    // Emergency rollback and forcing controls.
    'force_driver' => env('OA_RUNTIME_FORCE_DRIVER'),
    'rollback' => [
        'force_legacy' => (bool) env('OA_RUNTIME_FORCE_LEGACY', false),
    ],

    'elixir' => [
        'base_url' => rtrim((string) env('OA_RUNTIME_ELIXIR_BASE_URL', 'http://runtime:4000'), '/'),
        'stream_path' => (string) env('OA_RUNTIME_ELIXIR_STREAM_PATH', '/internal/v1/runs/stream'),
        'tools_execute_path' => (string) env('OA_RUNTIME_ELIXIR_TOOLS_EXECUTE_PATH', '/internal/v1/tools/execute'),
        'skills_tool_specs_path' => (string) env('OA_RUNTIME_ELIXIR_SKILLS_TOOL_SPECS_PATH', '/internal/v1/skills/tool-specs'),
        'skills_skill_specs_path' => (string) env('OA_RUNTIME_ELIXIR_SKILLS_SKILL_SPECS_PATH', '/internal/v1/skills/skill-specs'),
        'skills_publish_path_template' => (string) env('OA_RUNTIME_ELIXIR_SKILLS_PUBLISH_PATH_TEMPLATE', '/internal/v1/skills/skill-specs/{skill_id}/{version}/publish'),
        'skills_release_path_template' => (string) env('OA_RUNTIME_ELIXIR_SKILLS_RELEASE_PATH_TEMPLATE', '/internal/v1/skills/releases/{skill_id}/{version}'),
        'codex_workers_path' => (string) env('OA_RUNTIME_ELIXIR_CODEX_WORKERS_PATH', '/internal/v1/codex/workers'),
        'codex_worker_snapshot_path_template' => (string) env('OA_RUNTIME_ELIXIR_CODEX_WORKER_SNAPSHOT_PATH_TEMPLATE', '/internal/v1/codex/workers/{worker_id}/snapshot'),
        'codex_worker_stream_path_template' => (string) env('OA_RUNTIME_ELIXIR_CODEX_WORKER_STREAM_PATH_TEMPLATE', '/internal/v1/codex/workers/{worker_id}/stream'),
        'codex_worker_requests_path_template' => (string) env('OA_RUNTIME_ELIXIR_CODEX_WORKER_REQUESTS_PATH_TEMPLATE', '/internal/v1/codex/workers/{worker_id}/requests'),
        'codex_worker_events_path_template' => (string) env('OA_RUNTIME_ELIXIR_CODEX_WORKER_EVENTS_PATH_TEMPLATE', '/internal/v1/codex/workers/{worker_id}/events'),
        'codex_worker_stop_path_template' => (string) env('OA_RUNTIME_ELIXIR_CODEX_WORKER_STOP_PATH_TEMPLATE', '/internal/v1/codex/workers/{worker_id}/stop'),
        'health_path' => (string) env('OA_RUNTIME_ELIXIR_HEALTH_PATH', '/healthz'),

        // Bounded retries for runtime request establishment.
        'max_retries' => (int) env('OA_RUNTIME_ELIXIR_MAX_RETRIES', 2),
        'retry_backoff_ms' => (int) env('OA_RUNTIME_ELIXIR_RETRY_BACKOFF_MS', 200),

        // Timeouts in milliseconds.
        'connect_timeout_ms' => (int) env('OA_RUNTIME_ELIXIR_CONNECT_TIMEOUT_MS', 2_500),
        'timeout_ms' => (int) env('OA_RUNTIME_ELIXIR_TIMEOUT_MS', 60_000),

        // Internal request signing (HMAC SHA-256).
        'signing_key' => (string) env('OA_RUNTIME_SIGNING_KEY', ''),
        'signing_key_id' => (string) env('OA_RUNTIME_SIGNING_KEY_ID', 'runtime-v1'),
        'signature_ttl_seconds' => (int) env('OA_RUNTIME_SIGNATURE_TTL_SECONDS', 60),

        // Streaming read chunk size.
        'stream_chunk_bytes' => (int) env('OA_RUNTIME_ELIXIR_STREAM_CHUNK_BYTES', 1024),
    ],

    'internal' => [
        'shared_secret' => (string) env('OA_RUNTIME_INTERNAL_SHARED_SECRET', ''),
        'key_id' => (string) env('OA_RUNTIME_INTERNAL_KEY_ID', 'runtime-internal-v1'),
        'signature_ttl_seconds' => (int) env('OA_RUNTIME_INTERNAL_SIGNATURE_TTL_SECONDS', 60),
        'secret_fetch_path' => (string) env('OA_RUNTIME_INTERNAL_SECRET_FETCH_PATH', '/api/internal/runtime/integrations/secrets/fetch'),
        'secret_cache_ttl_ms' => (int) env('OA_RUNTIME_INTERNAL_SECRET_CACHE_TTL_MS', 60000),
    ],

    'comms' => [
        'resend' => [
            'webhook_secret' => (string) env('OA_RESEND_WEBHOOK_SECRET', ''),
            'webhook_tolerance_seconds' => (int) env('OA_RESEND_WEBHOOK_TOLERANCE_SECONDS', 300),
        ],
        'runtime_delivery_ingest_path' => (string) env('OA_RUNTIME_ELIXIR_COMMS_DELIVERY_INGEST_PATH', '/internal/v1/comms/delivery-events'),
        'runtime_delivery_timeout_ms' => (int) env('OA_RUNTIME_ELIXIR_COMMS_DELIVERY_TIMEOUT_MS', 10000),
        'runtime_delivery_max_retries' => (int) env('OA_RUNTIME_ELIXIR_COMMS_DELIVERY_MAX_RETRIES', 2),
        'runtime_delivery_retry_backoff_ms' => (int) env('OA_RUNTIME_ELIXIR_COMMS_DELIVERY_RETRY_BACKOFF_MS', 200),
    ],

    'shadow' => [
        'enabled' => (bool) env('OA_RUNTIME_SHADOW_ENABLED', false),
        'sample_rate' => (float) env('OA_RUNTIME_SHADOW_SAMPLE_RATE', 1.0),
        'max_capture_bytes' => (int) env('OA_RUNTIME_SHADOW_MAX_CAPTURE_BYTES', 200_000),
    ],

    'canary' => [
        // Deterministic canary routing percentages (0-100).
        'user_percent' => (int) env('OA_RUNTIME_CANARY_USER_PERCENT', 0),
        'autopilot_percent' => (int) env('OA_RUNTIME_CANARY_AUTOPILOT_PERCENT', 0),
        'seed' => (string) env('OA_RUNTIME_CANARY_SEED', 'runtime-canary-v1'),
    ],

    'overrides' => [
        'enabled' => (bool) env('OA_RUNTIME_OVERRIDES_ENABLED', true),
    ],
];
