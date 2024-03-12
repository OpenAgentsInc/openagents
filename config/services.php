<?php

return [

    'github' => [
        'client_id' => env('GITHUB_CLIENT_ID'),
        'client_secret' => env('GITHUB_CLIENT_SECRET'),
        'redirect' => '/github',
    ],

    'twitter' => [
        'client_id' => env('TWITTER_CLIENT_ID'),
        'client_secret' => env('TWITTER_CLIENT_SECRET'),
        'redirect' => '/twitter',
    ],

    'pdftotext' => [
        'path' => env('PDF_TO_TEXT_PATH', '/opt/homebrew/bin/pdftotext'),
    ],

];
