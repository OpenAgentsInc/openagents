<?php

$environment = strtolower((string) env('APP_ENV', 'production'));
$disabledByDefault = in_array($environment, ['local', 'development', 'dev', 'testing', 'staging'], true);

return [
    'api_key' => env('POSTHOG_API_KEY', ''),
    'host' => env('POSTHOG_HOST', 'https://us.i.posthog.com'),
    'disabled' => filter_var(
        (string) env('POSTHOG_DISABLED', $disabledByDefault ? 'true' : 'false'),
        FILTER_VALIDATE_BOOL,
    ),
    'debug' => filter_var((string) env('APP_DEBUG', 'false'), FILTER_VALIDATE_BOOL),
];
