<?php

use App\AI\BedrockMessageConverter;

beforeEach(function () {
    $this->converter = new BedrockMessageConverter();
});

test('converts simple user message', function () {
    $messages = [
        [
            'role' => 'user',
            'content' => 'Hello'
        ]
    ];

    $result = $this->converter->convertToBedrockChatMessages($messages);

    expect($result)->toBe([
        'system' => null,
        'messages' => [
            [
                'role' => 'user',
                'content' => [
                    ['text' => 'Hello']
                ]
            ]
        ]
    ]);
});

test('converts user and assistant messages', function () {
    $messages = [
        [
            'role' => 'user',
            'content' => 'Hello'
        ],
        [
            'role' => 'assistant',
            'content' => 'Hi there'
        ]
    ];

    $result = $this->converter->convertToBedrockChatMessages($messages);

    expect($result)->toBe([
        'system' => null,
        'messages' => [
            [
                'role' => 'user',
                'content' => [
                    ['text' => 'Hello']
                ]
            ],
            [
                'role' => 'assistant',
                'content' => [
                    ['text' => 'Hi there']
                ]
            ],
            [
                'role' => 'user',
                'content' => [
                    ['text' => 'Continue.']
                ]
            ]
        ]
    ]);
});

test('converts system message', function () {
    $messages = [
        [
            'role' => 'system',
            'content' => 'You are a helpful assistant'
        ],
        [
            'role' => 'user',
            'content' => 'Hello'
        ]
    ];

    $result = $this->converter->convertToBedrockChatMessages($messages);

    expect($result)->toBe([
        'system' => 'You are a helpful assistant',
        'messages' => [
            [
                'role' => 'user',
                'content' => [
                    ['text' => 'Hello']
                ]
            ]
        ]
    ]);
});

test('throws exception for assistant first message', function () {
    $messages = [
        [
            'role' => 'assistant',
            'content' => 'Hi there'
        ],
        [
            'role' => 'user',
            'content' => 'Hello'
        ]
    ];

    expect(fn() => $this->converter->convertToBedrockChatMessages($messages))
        ->toThrow(Exception::class, 'A conversation must start with a user message (after any system messages).');
});

test('converts tool results', function () {
    $messages = [
        [
            'role' => 'user',
            'content' => 'Show me the README'
        ],
        [
            'role' => 'assistant',
            'content' => [
                [
                    'type' => 'text',
                    'text' => 'I\'ll help you with that.'
                ],
                [
                    'type' => 'tool-call',
                    'toolCallId' => 'tool123',
                    'toolName' => 'view_file',
                    'args' => [
                        'path' => 'README.md'
                    ]
                ]
            ],
            'toolInvocations' => [
                [
                    'state' => 'result',
                    'toolCallId' => 'tool123',
                    'toolName' => 'view_file',
                    'result' => [
                        'content' => 'README content here'
                    ]
                ]
            ]
        ]
    ];

    $result = $this->converter->convertToBedrockChatMessages($messages);

    expect($result)->toBe([
        'system' => null,
        'messages' => [
            [
                'role' => 'user',
                'content' => [
                    ['text' => 'Show me the README']
                ]
            ],
            [
                'role' => 'assistant',
                'content' => [
                    ['text' => 'I\'ll help you with that.'],
                    [
                        'toolUse' => [
                            'toolUseId' => 'tool123',
                            'name' => 'view_file',
                            'input' => [
                                'path' => 'README.md'
                            ]
                        ]
                    ]
                ]
            ],
            [
                'role' => 'user',
                'content' => [
                    [
                        'toolResult' => [
                            'toolUseId' => 'tool123',
                            'content' => [['text' => json_encode([
                                'content' => 'README content here'
                            ])]]
                        ]
                    ]
                ]
            ]
        ]
    ]);
});