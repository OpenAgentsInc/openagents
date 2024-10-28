<?php

namespace Tests\Unit;

use App\AI\Traits\BedrockMessageFormatting;
use PHPUnit\Framework\TestCase;

class BedrockMessageFormattingTest extends TestCase
{
    use BedrockMessageFormatting;

    public function test_format_response_with_text_only()
    {
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

        $result = $this->formatResponse($decodedBody);

        $this->assertEquals([
            'content' => 'Hello world',
            'input_tokens' => 10,
            'output_tokens' => 20,
            'toolInvocations' => []
        ], $result);
    }

    public function test_format_response_with_tool_use()
    {
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

        $result = $this->formatResponse($decodedBody);

        $this->assertEquals([
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
        ], $result);
    }

    public function test_format_response_with_tool_result()
    {
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

        $result = $this->formatResponse($decodedBody);

        $this->assertEquals([
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
        ], $result);
    }

    public function test_format_response_with_tool_error()
    {
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

        $result = $this->formatResponse($decodedBody);

        $this->assertEquals([
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
        ], $result);
    }

    public function test_determine_tool_result_status()
    {
        // Test explicit success
        $this->assertEquals('success', $this->determineToolResultStatus(['success' => true]));
        $this->assertEquals('error', $this->determineToolResultStatus(['success' => false]));

        // Test error indicators
        $this->assertEquals('error', $this->determineToolResultStatus(['error' => 'Something went wrong']));
        $this->assertEquals('error', $this->determineToolResultStatus(['errorMessage' => 'Something went wrong']));

        // Test content presence
        $this->assertEquals('success', $this->determineToolResultStatus(['content' => 'Some content']));
        $this->assertEquals('error', $this->determineToolResultStatus(['content' => '']));

        // Test default case
        $this->assertEquals('error', $this->determineToolResultStatus([]));
    }

    public function test_format_response_with_multiple_tool_calls()
    {
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

        $result = $this->formatResponse($decodedBody);

        $this->assertEquals([
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
        ], $result);
    }
}