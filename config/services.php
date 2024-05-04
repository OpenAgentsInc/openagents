<?php

return [
    /*
    |--------------------------------------------------------------------------
    | Third Party Services
    |--------------------------------------------------------------------------
    |
    | This file is for storing the credentials for third party services such
    | as Mailgun, Postmark, AWS and more. This file provides the de facto
    | location for this type of information, allowing packages to have
    | a conventional file to locate the various service credentials.
    |
    */

    'twitter' => [
        'client_id' => env('X_CLIENT_ID'),
        'client_secret' => env('X_CLIENT_SECRET'),
        'redirect' => '/callback/x',
    ],

    'github' => [
        'token' => env('GITHUB_TOKEN'),
    ],

    'greptile' => [
        'api_key' => env('GREPTILE_API_KEY'),
    ],

    'huggingface' => [
        'api_key' => env('HUGGINGFACE_API_KEY'),
    ],
    'openai' => [
        'api_key' => env('OPENAI_API_KEY'),
    ],
];
