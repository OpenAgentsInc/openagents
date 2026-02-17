<?php

return [
    'l402' => [
        // Optional host allowlist gate. Off by default to avoid blocking valid
        // L402 endpoints (for example api.sats4ai.com) unless explicitly enabled.
        'enforce_host_allowlist' => (bool) env('L402_ENFORCE_HOST_ALLOWLIST', false),

        // Comma-separated list of hostnames that are allowed for paid fetches.
        // Used only when enforce_host_allowlist is true (or when a per-autopilot
        // policy provides explicit allowed hosts).
        'allowlist_hosts' => array_values(array_filter(array_map(
            fn (string $h) => trim($h),
            explode(',', (string) env('L402_ALLOWLIST_HOSTS', 'sats4ai.com,l402.openagents.com'))
        ))),

        // Cache (macaroon+preimage) TTL. Keep low until we're confident about revocation semantics.
        'credential_ttl_seconds' => (int) env('L402_CREDENTIAL_TTL_SECONDS', 600),

        // Response capture limits.
        'response_max_bytes' => (int) env('L402_RESPONSE_MAX_BYTES', 65536),
        'response_preview_bytes' => (int) env('L402_RESPONSE_PREVIEW_BYTES', 1024),

        // Invoice payment timeout (ms).
        'payment_timeout_ms' => (int) env('L402_PAYMENT_TIMEOUT_MS', 12000),

        // Approval intent TTL (seconds) for queued payment tasks.
        'approval_ttl_seconds' => (int) env('L402_APPROVAL_TTL_SECONDS', 600),

        // Which invoice payer to use: "spark_wallet" | "lnd_rest" | "fake".
        'invoice_payer' => (string) env('L402_INVOICE_PAYER', 'spark_wallet'),
    ],

    'lnd_rest' => [
        'base_url' => env('LND_REST_BASE_URL'),
        'macaroon_hex' => env('LND_REST_MACAROON_HEX'),
        // Optional base64 TLS cert for LND REST self-signed setups.
        'tls_cert_base64' => env('LND_REST_TLS_CERT_BASE64'),
        // Set to "false" only for local experiments.
        'tls_verify' => env('LND_REST_TLS_VERIFY', true),
    ],

    'spark_executor' => [
        'base_url' => env('SPARK_EXECUTOR_BASE_URL', env('OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL')),
        'auth_token' => env('SPARK_EXECUTOR_AUTH_TOKEN', env('OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN')),
        'timeout_ms' => (int) env('SPARK_EXECUTOR_TIMEOUT_MS', 20000),
    ],

    'agent_wallets' => [
        'wallet_id_prefix' => (string) env('SPARK_AGENT_WALLET_ID_PREFIX', 'oa-user-'),
    ],

    'operator' => [
        'aperture_config_path' => (string) env('L402_APERTURE_CONFIG_PATH', storage_path('app/l402/aperture-paywalls.json')),
        'aperture_reconcile_command' => env('L402_APERTURE_RECONCILE_COMMAND'),
        'aperture_reconcile_timeout_seconds' => (int) env('L402_APERTURE_RECONCILE_TIMEOUT_SECONDS', 120),
    ],

    // Demo presets (EP212). Keep endpoints here so demos are reproducible.
    'demo_presets' => [
        'sats4ai' => [
            'url' => (string) env('L402_DEMO_SATS4AI_URL', 'https://sats4ai.com/api/l402/text-generation'),
            'method' => 'POST',
            'headers' => [
                'Content-Type' => 'application/json',
            ],
            'body' => '{"input":[{"role":"User","content":"Tell me one short fact about Bitcoin."}],"model":"Best"}',
            'scope' => 'ep212.sats4ai',
        ],

        'ep212_openagents_premium' => [
            'url' => (string) env('L402_DEMO_EP212_OPENAGENTS_PREMIUM_URL', 'https://l402.openagents.com/ep212/premium-signal'),
            'method' => 'GET',
            'headers' => [
                'Accept' => 'application/json',
            ],
            'scope' => 'ep212.openagents.premium',
        ],

        'ep212_openagents_expensive' => [
            'url' => (string) env('L402_DEMO_EP212_OPENAGENTS_EXPENSIVE_URL', 'https://l402.openagents.com/ep212/expensive-signal'),
            'method' => 'GET',
            'headers' => [
                'Accept' => 'application/json',
            ],
            'scope' => 'ep212.openagents.expensive',
        ],

        // In-process fake seller used for deterministic demos/tests. No network required.
        'fake' => [
            'url' => 'https://fake-l402.local/premium',
            'method' => 'POST',
            'headers' => [
                'Content-Type' => 'application/json',
            ],
            'body' => '{"hello":"world"}',
            'scope' => 'demo.fake',
        ],
    ],
];
