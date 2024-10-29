```php
$result = $client->converse([
    'additionalModelRequestFields' => [
    ],
    'additionalModelResponseFieldPaths' => ['<string>', ...],
    'guardrailConfig' => [
        'guardrailIdentifier' => '<string>', // REQUIRED
        'guardrailVersion' => '<string>', // REQUIRED
        'trace' => 'enabled|disabled',
    ],
    'inferenceConfig' => [
        'maxTokens' => <integer>,
        'stopSequences' => ['<string>', ...],
        'temperature' => <float>,
        'topP' => <float>,
    ],
    'messages' => [ // REQUIRED
        [
            'content' => [ // REQUIRED
                [
                    'document' => [
                        'format' => 'pdf|csv|doc|docx|xls|xlsx|html|txt|md', // REQUIRED
                        'name' => '<string>', // REQUIRED
                        'source' => [ // REQUIRED
                            'bytes' => <string || resource || Psr\Http\Message\StreamInterface>,
                        ],
                    ],
                    'guardContent' => [
                        'text' => [
                            'qualifiers' => ['<string>', ...],
                            'text' => '<string>', // REQUIRED
                        ],
                    ],
                    'image' => [
                        'format' => 'png|jpeg|gif|webp', // REQUIRED
                        'source' => [ // REQUIRED
                            'bytes' => <string || resource || Psr\Http\Message\StreamInterface>,
                        ],
                    ],
                    'text' => '<string>',
                    'toolResult' => [
                        'content' => [ // REQUIRED
                            [
                                'document' => [
                                    'format' => 'pdf|csv|doc|docx|xls|xlsx|html|txt|md', // REQUIRED
                                    'name' => '<string>', // REQUIRED
                                    'source' => [ // REQUIRED
                                        'bytes' => <string || resource || Psr\Http\Message\StreamInterface>,
                                    ],
                                ],
                                'image' => [
                                    'format' => 'png|jpeg|gif|webp', // REQUIRED
                                    'source' => [ // REQUIRED
                                        'bytes' => <string || resource || Psr\Http\Message\StreamInterface>,
                                    ],
                                ],
                                'json' => [
                                ],
                                'text' => '<string>',
                            ],
                            // ...
                        ],
                        'status' => 'success|error',
                        'toolUseId' => '<string>', // REQUIRED
                    ],
                    'toolUse' => [
                        'input' => [ // REQUIRED
                        ],
                        'name' => '<string>', // REQUIRED
                        'toolUseId' => '<string>', // REQUIRED
                    ],
                ],
                // ...
            ],
            'role' => 'user|assistant', // REQUIRED
        ],
        // ...
    ],
    'modelId' => '<string>', // REQUIRED
    'system' => [
        [
            'guardContent' => [
                'text' => [
                    'qualifiers' => ['<string>', ...],
                    'text' => '<string>', // REQUIRED
                ],
            ],
            'text' => '<string>',
        ],
        // ...
    ],
    'toolConfig' => [
        'toolChoice' => [
            'any' => [
            ],
            'auto' => [
            ],
            'tool' => [
                'name' => '<string>', // REQUIRED
            ],
        ],
        'tools' => [ // REQUIRED
            [
                'toolSpec' => [
                    'description' => '<string>',
                    'inputSchema' => [ // REQUIRED
                        'json' => [
                        ],
                    ],
                    'name' => '<string>', // REQUIRED
                ],
            ],
            // ...
        ],
    ],
]);
```
