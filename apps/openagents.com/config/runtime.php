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
        'base_url' => rtrim((string) env('OA_RUNTIME_ELIXIR_BASE_URL', 'http://openagents-runtime:4000'), '/'),
        'stream_path' => (string) env('OA_RUNTIME_ELIXIR_STREAM_PATH', '/internal/v1/runs/stream'),
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
