<?php

return [
    'address' => env('POOL_ADDRESS') ?: env('NOSTR_POOL'),
    'address_ssl' => env('POOL_ADDRESS_SSL', true) ?: env('NOSTR_POOL_SSL'),
    'webhook_secret' => env('POOL_WEBHOOK_SECRET') ?: env('NOSTR_WEBHOOK_SECRET'),
    'encrypt' => env('POOL_ENCRYPT') ?: env('NOSTR_ENCRYPT'),
    'node_token' => env('POOL_NODE_TOKEN') ?: env('NOSTR_NODE_TOKEN'),
];
