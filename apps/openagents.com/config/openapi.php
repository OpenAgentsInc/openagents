<?php

return [

    'collections' => [

        'default' => [

            'info' => [
                'title' => 'OpenAgents API',
                'description' => 'OpenAgents REST API (Laravel). Primary auth is Sanctum bearer tokens for /api endpoints.',
                'version' => env('APP_VERSION', '1.0.0'),
                'contact' => [
                    'name' => 'OpenAgents',
                    'url' => 'https://openagents.com',
                ],
            ],

            'servers' => [
                [
                    'url' => env('APP_URL', 'https://openagents.com'),
                    'description' => 'Primary production server',
                    'variables' => [],
                ],
            ],

            'tags' => [
                [
                    'name' => 'Auth',
                    'description' => 'Authentication context and token lifecycle endpoints.',
                ],
                [
                    'name' => 'Chat',
                    'description' => 'Conversation, run, and stream endpoints.',
                ],
                [
                    'name' => 'Autopilot',
                    'description' => 'Autopilot resources, thread scoping, and stream alias routes.',
                ],
                [
                    'name' => 'Profile',
                    'description' => 'Authenticated profile management endpoints.',
                ],
                [
                    'name' => 'Shouts',
                    'description' => 'Global public broadcast messages, optionally scoped by zone.',
                ],
                [
                    'name' => 'Whispers',
                    'description' => 'Direct messages between two users, inbox and thread retrieval.',
                ],
                [
                    'name' => 'L402',
                    'description' => 'Lightning L402 wallet, receipts, and deployment telemetry endpoints.',
                ],
                [
                    'name' => 'Agent Payments',
                    'description' => 'Per-user Spark wallet lifecycle and payment APIs (invoice + pay + spark transfers).',
                ],
            ],

            'security' => [
                \GoldSpecDigital\ObjectOrientedOAS\Objects\SecurityRequirement::create()->securityScheme('SanctumToken'),
            ],

            // Non standard attributes used by code/doc generation tools can be added here
            'extensions' => [
                'x-generatedBy' => 'vyuldashev/laravel-openapi',
            ],

            // Route for exposing specification.
            // Leave uri null to disable.
            // We serve /openapi.json via App\Http\Controllers\OpenApiSpecController to guarantee minified JSON.
            'route' => [
                'uri' => null,
                'middleware' => [],
            ],

            // Register custom middlewares for different objects.
            'middlewares' => [
                'paths' => [
                    //
                ],
                'components' => [
                    //
                ],
            ],

        ],

    ],

    // Directories to use for locating OpenAPI object definitions.
    'locations' => [
        'callbacks' => [
            app_path('OpenApi/Callbacks'),
        ],

        'request_bodies' => [
            app_path('OpenApi/RequestBodies'),
        ],

        'responses' => [
            app_path('OpenApi/Responses'),
        ],

        'schemas' => [
            app_path('OpenApi/Schemas'),
        ],

        'security_schemes' => [
            app_path('OpenApi/SecuritySchemes'),
        ],
    ],

];
