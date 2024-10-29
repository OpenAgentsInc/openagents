<?php

use App\AI\Traits\BedrockMessageFormatting;

class TestClass
{
    use BedrockMessageFormatting;

    public function publicFormatResponse(array $decodedBody): array
    {
        return $this->formatResponse($decodedBody);
    }
}

beforeEach(function () {
    $this->formatter = new TestClass();
});

it('stops processing text after tool use', function () {
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
                    ['text' => 'I apologize for the error. '],
                    ['text' => 'Let me try something else. ']
                ]
            ]
        ],
        'usage' => [
            'inputTokens' => 100,
            'outputTokens' => 50
        ]
    ];

    $result = $this->formatter->publicFormatResponse($decodedBody);

    expect($result['content'])->toBe('Let me help you with that. ')
        ->and($result['toolInvocations'])
        ->toHaveCount(1)
        ->and($result['toolInvocations'][0]['toolName'])
        ->toBe('view_file');
});

it('handles multiple tool uses while ignoring intermediate text', function () {
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
                    ['text' => 'I apologize for the error. '],
                    [
                        'toolUse' => [
                            'toolUseId' => 'tool124',
                            'name' => 'view_file',
                            'input' => ['path' => 'CONTRIBUTING.md']
                        ]
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

    expect($result['content'])->toBe('Let me help you with that. ')
        ->and($result['toolInvocations'])->toHaveCount(2)
        ->and($result['toolInvocations'][0]['toolName'])->toBe('view_file')
        ->and($result['toolInvocations'][1]['toolName'])->toBe('view_file');
});

it('processes tool results correctly', function () {
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
                    ['text' => 'Here is what I found: ']
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
        ]
    ];

    $result = $this->formatter->publicFormatResponse($decodedBody);

    expect($result['content'])->toBe('Let me check that file. ')
        ->and($result['toolInvocations'])->toHaveCount(1)
        ->and($result['toolInvocations'][0])->toHaveKeys(['toolName', 'toolCallId', 'args', 'result'])
        ->and($result['toolInvocations'][0]['result']['value']['result']['success'])->toBeTrue();
});

it('handles tool errors correctly', function () {
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
                    ['text' => 'I apologize for the error. ']
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
        ]
    ];

    $result = $this->formatter->publicFormatResponse($decodedBody);

    expect($result['content'])->toBe('Let me check that file. ')
        ->and($result['toolInvocations'])->toHaveCount(1)
        ->and($result['toolInvocations'][0])->toHaveKeys(['toolName', 'toolCallId', 'args', 'result'])
        ->and($result['toolInvocations'][0]['result']['value']['result']['success'])->toBeFalse()
        ->and($result['toolInvocations'][0]['result']['value']['result']['error'])->toBe('File not found');
});