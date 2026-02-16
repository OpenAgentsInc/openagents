<?php

return [
    'l402' => [
        // Comma-separated list of hostnames that are allowed for paid fetches.
        // This is enforced server-side regardless of tool input.
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

        // Which invoice payer to use: "lnd_rest" | "fake".
        'invoice_payer' => (string) env('L402_INVOICE_PAYER', 'fake'),
    ],

    'lnd_rest' => [
        'base_url' => env('LND_REST_BASE_URL'),
        'macaroon_hex' => env('LND_REST_MACAROON_HEX'),
        // Optional base64 TLS cert for LND REST self-signed setups.
        'tls_cert_base64' => env('LND_REST_TLS_CERT_BASE64'),
        // Set to "false" only for local experiments.
        'tls_verify' => env('LND_REST_TLS_VERIFY', true),
    ],

    // Demo presets (Phase 5). Keep endpoints here so demos are reproducible.
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
