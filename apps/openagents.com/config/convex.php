<?php

return [
    'token' => [
        /*
        |--------------------------------------------------------------------------
        | Convex Auth Token Bridge
        |--------------------------------------------------------------------------
        |
        | Laravel mints short-lived Convex auth JWTs from authenticated OA session
        | identity. Convex validates these claims via its custom JWT auth config.
        |
        */
        'enabled' => (bool) env('OA_CONVEX_TOKEN_ENABLED', true),
        'issuer' => (string) env('OA_CONVEX_TOKEN_ISSUER', (string) env('APP_URL', '')),
        'audience' => (string) env('OA_CONVEX_TOKEN_AUDIENCE', 'openagents-convex'),
        'ttl_seconds' => (int) env('OA_CONVEX_TOKEN_TTL_SECONDS', 300),
        'subject_prefix' => (string) env('OA_CONVEX_TOKEN_SUBJECT_PREFIX', 'user'),
        'key_id' => (string) env('OA_CONVEX_TOKEN_KEY_ID', 'convex-auth-v1'),

        // HS256 symmetric signing key (server-only secret).
        'signing_key' => (string) env('OA_CONVEX_TOKEN_SIGNING_KEY', ''),
    ],
];
