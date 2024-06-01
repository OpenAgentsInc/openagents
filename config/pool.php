<?php

return [
    'address' => env('POOL_ADDRESS'),
    'address_ssl' => env('POOL_ADDRESS_SSL', true),
    'webhook_secret' => env('POOL_WEBHOOK_SECRET'),
    'encrypt' => env('POOL_ENCRYPT'),
    'node_token' => env('POOL_NODE_TOKEN'),
];
