<?php

return [
    'api_key' => env('POSTHOG_API_KEY', ''),
    'host' => env('POSTHOG_HOST', 'https://us.i.posthog.com'),
    'disabled' => env('POSTHOG_DISABLED', false),
    'debug' => env('APP_DEBUG', false),
];
