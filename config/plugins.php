<?php

return [
    'secret' => env('PLUGINS_SECRET') ?: (env('POOL_WEBHOOK_SECRET') ?: env('NOSTR_WEBHOOK_SECRET')),
];
