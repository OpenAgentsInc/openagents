<?php

use App\AI\Traits\BedrockMessageFormatting;

class TestClass
{
    use BedrockMessageFormatting;
}

beforeEach(function () {
    $this->formatter = new TestClass();
});

test('formats response with text only', function () {
    $decodedBody = [
        'output' => [
            'message' => [
                'content' => [
                    ['text' => 'Hello'],
                    ['text' => ' world']
                ]
            ]
        ],
        'usage' => [
            'inputTokens' => 10,
            'outputTokens' => 20
        ]
    ];

    $result = $this->formatter->formatResponse($decodedBody);

    expect($result)->toBe([
        'content' => 'Hello world',
        'input_tokens' => 10,
        'output_tokens' => 20,
        'toolInvocations' => []
    ]);
});

test('formats response with tool use', function () {
    $decodedBody = [
        'output' => [
            'message' => [
                'content' => [
                    ['text' => 'Let me check that file for you.'],
                    [
                        'toolUse' => [
                            'name' => 'view_file',
                            'toolUseId' => 'tool123',
                            'input' => ['path' => 'README.md']
                        ]
                    ]
                ]
            ]
        ],
        'usage' => [
            'inputTokens' => 15,
            'outputTokens' => 25
        ]
    ];

    $result = $this->formatter->formatResponse($decodedBody);

    expect($result)->toBe([
        'content' => 'Let me check that file for you.',
        'input_tokens' => 15,
        'output_tokens' => 25,
        'toolInvocations' => [
            [
                'toolName' => 'view_file',
                'toolCallId' => 'tool123',
                'args' => ['path' => 'README.md']
            ]
        ]
    ]);
});

test('formats response with tool result', function () {
    $decodedBody = [
        'output' => [
            'message' => [
                'content' => [
                    ['text' => 'Here\'s what I found:'],
                    [
                        'toolUse' => [
                            'name' => 'view_file',
                            'toolUseId' => 'tool123',
                            'input' => ['path' => 'README.md']
                        ]
                    ]
                ],
                'toolResults' => [
                    [
                        'toolUseId' => 'tool123',
                        'status' => 'success',
                        'content' => [
                            ['text' => '{"content": "# README\n\nThis is a test file"}']
                        ]
                    ]
                ]
            ]
        ],
        'usage' => [
            'inputTokens' => 20,
            'outputTokens' => 30
        ]
    ];

    $result = $this->formatter->formatResponse($decodedBody);

    expect($result)->toBe([
        'content' => 'Here\'s what I found:',
        'input_tokens' => 20,
        'output_tokens' => 30,
        'toolInvocations' => [
            [
                'toolName' => 'view_file',
                'toolCallId' => 'tool123',
                'args' => ['path' => 'README.md'],
                'result' => [
                    'type' => 'tool_call',
                    'value' => [
                        'toolCallId' => 'tool123',
                        'toolName' => 'view_file',
                        'args' => ['path' => 'README.md'],
                        'result' => [
                            'success' => true,
                            'content' => '{"content": "# README\n\nThis is a test file"}',
                            'error' => null
                        ]
                    ]
                ]
            ]
        ]
    ]);
});

test('formats response with tool error', function () {
    $decodedBody = [
        'output' => [
            'message' => [
                'content' => [
                    ['text' => 'Let me try to access that file:'],
                    [
                        'toolUse' => [
                            'name' => 'view_file',
                            'toolUseId' => 'tool123',
                            'input' => ['path' => 'nonexistent.md']
                        ]
                    ]
                ],
                'toolResults' => [
                    [
                        'toolUseId' => 'tool123',
                        'status' => 'error',
                        'content' => [
                            ['text' => 'File not found']
                        ]
                    ]
                ]
            ]
        ],
        'usage' => [
            'inputTokens' => 25,
            'outputTokens' => 35
        ]
    ];

    $result = $this->formatter->formatResponse($decodedBody);

    expect($result)->toBe([
        'content' => 'Let me try to access that file:',
        'input_tokens' => 25,
        'output_tokens' => 35,
        'toolInvocations' => [
            [
                'toolName' => 'view_file',
                'toolCallId' => 'tool123',
                'args' => ['path' => 'nonexistent.md'],
                'result' => [
                    'type' => 'tool_call',
                    'value' => [
                        'toolCallId' => 'tool123',
                        'toolName' => 'view_file',
                        'args' => ['path' => 'nonexistent.md'],
                        'result' => [
                            'success' => false,
                            'content' => null,
                            'error' => 'File not found'
                        ]
                    ]
                ]
            ]
        ]
    ]);
});

test('determines tool result status correctly', function () {
    // Test explicit success
    expect($this->formatter->determineToolResultStatus(['success' => true]))->toBe('success');
    expect($this->formatter->determineToolResultStatus(['success' => false]))->toBe('error');

    // Test error indicators
    expect($this->formatter->determineToolResultStatus(['error' => 'Something went wrong']))->toBe('error');
    expect($this->formatter->determineToolResultStatus(['errorMessage' => 'Something went wrong']))->toBe('error');

    // Test content presence
    expect($this->formatter->determineToolResultStatus(['content' => 'Some content']))->toBe('success');
    expect($this->formatter->determineToolResultStatus(['content' => '']))->toBe('error');

    // Test default case
    expect($this->formatter->determineToolResultStatus([]))->toBe('error');
});

test('formats response with multiple tool calls', function () {
    $decodedBody = [
        'output' => [
            'message' => [
                'content' => [
                    ['text' => 'Let me check multiple files:'],
                    [
                        'toolUse' => [
                            'name' => 'view_file',
                            'toolUseId' => 'tool123',
                            'input' => ['path' => 'README.md']
                        ]
                    ],
                    [
                        'toolUse' => [
                            'name' => 'view_file',
                            'toolUseId' => 'tool124',
                            'input' => ['path' => 'CONTRIBUTING.md']
                        ]
                    ]
                ],
                'toolResults' => [
                    [
                        'toolUseId' => 'tool123',
                        'status' => 'success',
                        'content' => [
                            ['text' => '{"content": "# README"}']
                        ]
                    ],
                    [
                        'toolUseId' => 'tool124',
                        'status' => 'success',
                        'content' => [
                            ['text' => '{"content": "# Contributing"}']
                        ]
                    ]
                ]
            ]
        ],
        'usage' => [
            'inputTokens' => 30,
            'outputTokens' => 40
        ]
    ];

    $result = $this->formatter->formatResponse($decodedBody);

    expect($result)->toBe([
        'content' => 'Let me check multiple files:',
        'input_tokens' => 30,
        'output_tokens' => 40,
        'toolInvocations' => [
            [
                'toolName' => 'view_file',
                'toolCallId' => 'tool123',
                'args' => ['path' => 'README.md'],
                'result' => [
                    'type' => 'tool_call',
                    'value' => [
                        'toolCallId' => 'tool123',
                        'toolName' => 'view_file',
                        'args' => ['path' => 'README.md'],
                        'result' => [
                            'success' => true,
                            'content' => '{"content": "# README"}',
                            'error' => null
                        ]
                    ]
                ]
            ],
            [
                'toolName' => 'view_file',
                'toolCallId' => 'tool124',
                'args' => ['path' => 'CONTRIBUTING.md'],
                'result' => [
                    'type' => 'tool_call',
                    'value' => [
                        'toolCallId' => 'tool124',
                        'toolName' => 'view_file',
                        'args' => ['path' => 'CONTRIBUTING.md'],
                        'result' => [
                            'success' => true,
                            'content' => '{"content": "# Contributing"}',
                            'error' => null
                        ]
                    ]
                ]
            ]
        ]
    ]);
});