<?php

use App\AI\Traits\BedrockMessageFormatting;

class TestClass
{
    use BedrockMessageFormatting;

    // Add public methods to expose protected methods for testing
    public function publicFormatResponse(array $decodedBody): array
    {
        return $this->formatResponse($decodedBody);
    }
}

beforeEach(function () {
    $this->formatter = new TestClass();
});

test('stops processing text after tool use', function () {
    $decodedBody = [
        'output' => [
            'message' => [
                'content' => [
                    ['text' => 'Let me help you with that. '],
                    [
                        'toolUse' => [
                            'toolUseId' => 'tool123',
                            'name' => 'view_file',
                            'input' => ['path' => 'README.md']
                        ]
                    ],
                    ['text' => 'I apologize for the error. '], // This should be ignored
                    ['text' => 'Let me try something else. '] // This should be ignored
                ]
            ]
        ],
        'usage' => [
            'inputTokens' => 100,
            'outputTokens' => 50
        ]
    ];

    $result = $this->formatter->publicFormatResponse($decodedBody);

    // Should only include text before the tool use
    expect($result['content'])->toBe('Let me help you with that. ');

    // Should include the tool invocation
    expect($result['toolInvocations'])->toHaveCount(1);
    expect($result['toolInvocations'][0]['toolName'])->toBe('view_file');
});

test('handles multiple tool uses while ignoring intermediate text', function () {
    $decodedBody = [
        'output' => [
            'message' => [
                'content' => [
                    ['text' => 'Let me help you with that. '],
                    [
                        'toolUse' => [
                            'toolUseId' => 'tool123',
                            'name' => 'view_file',
                            'input' => ['path' => 'README.md']
                        ]
                    ],
                    ['text' => 'I apologize for the error. '], // This should be ignored
                    [
                        'toolUse' => [
                            'toolUseId' => 'tool124',
                            'name' => 'view_file',
                            'input' => ['path' => 'CONTRIBUTING.md']
                        ]
                    ]
                ]
            ],
            'usage' => [
                'inputTokens' => 100,
                'outputTokens' => 50
            ]
        ];

    $result = $this->formatter->publicFormatResponse($decodedBody);

    // Should only include text before the first tool use
    expect($result['content'])->toBe('Let me help you with that. ');

    // Should include both tool invocations
    expect($result['toolInvocations'])->toHaveCount(2);
    expect($result['toolInvocations'][0]['toolName'])->toBe('view_file');
    expect($result['toolInvocations'][1]['toolName'])->toBe('view_file');
});

test('processes tool results correctly', function () {
    $decodedBody = [
        'output' => [
            'message' => [
                'content' => [
                    ['text' => 'Let me check that file. '],
                    [
                        'toolUse' => [
                            'toolUseId' => 'tool123',
                            'name' => 'view_file',
                            'input' => ['path' => 'README.md']
                        ]
                    ],
                    ['text' => 'Here is what I found: '] // This should be ignored
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
            ],
            'usage' => [
                'inputTokens' => 100,
                'outputTokens' => 50
            ]
        ];

    $result = $this->formatter->publicFormatResponse($decodedBody);

    // Should only include text before the tool use
    expect($result['content'])->toBe('Let me check that file. ');

    // Should include the tool invocation and its result
    expect($result['toolInvocations'])->toHaveCount(1);
    expect($result['toolInvocations'][0])->toHaveKeys(['toolName', 'toolCallId', 'args', 'result']);
    expect($result['toolInvocations'][0]['result']['value']['result']['success'])->toBeTrue();
});

test('handles tool errors correctly', function () {
    $decodedBody = [
        'output' => [
            'message' => [
                'content' => [
                    ['text' => 'Let me check that file. '],
                    [
                        'toolUse' => [
                            'toolUseId' => 'tool123',
                            'name' => 'view_file',
                            'input' => ['path' => 'README.md']
                        ]
                    ],
                    ['text' => 'I apologize for the error. '] // This should be ignored
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
            ],
            'usage' => [
                'inputTokens' => 100,
                'outputTokens' => 50
            ]
        ];

    $result = $this->formatter->publicFormatResponse($decodedBody);

    // Should only include text before the tool use
    expect($result['content'])->toBe('Let me check that file. ');

    // Should include the tool invocation and its error result
    expect($result['toolInvocations'])->toHaveCount(1);
    expect($result['toolInvocations'][0])->toHaveKeys(['toolName', 'toolCallId', 'args', 'result']);
    expect($result['toolInvocations'][0]['result']['value']['result']['success'])->toBeFalse();
    expect($result['toolInvocations'][0]['result']['value']['result']['error'])->toBe('File not found');
});