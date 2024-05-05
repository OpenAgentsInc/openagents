<?php

return [
    'pool' => env('NOSTR_POOL'),
    'pool_ssl' => env('NOSTR_POOL_SSL')=="true",
    'webhook_secret' => env('NOSTR_WEBHOOK_SECRET'),
    'encrypt_for' => env('NOSTR_ENCRYPT'),
    'node_token' => env('NOSTR_NODE_TOKEN'),
];
