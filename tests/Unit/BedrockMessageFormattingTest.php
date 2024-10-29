<?php

namespace Tests\Unit;

use App\AI\Traits\BedrockMessageFormatting;
use PHPUnit\Framework\TestCase;

class BedrockMessageFormattingTest extends TestCase
{
    use BedrockMessageFormatting;

    public function testFormatResponseStopsAfterToolUse()
    {
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

        $result = $this->formatResponse($decodedBody);

        $this->assertEquals(
            'Let me help you with that. ',
            $result['content'],
            'Response should only include text before the tool use'
        );

        $this->assertCount(
            1,
            $result['toolInvocations'],
            'Response should include the tool invocation'
        );

        $this->assertEquals(
            'view_file',
            $result['toolInvocations'][0]['toolName'],
            'Tool invocation should have correct name'
        );
    }

    public function testFormatResponseHandlesMultipleToolUses()
    {
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
                ]
            ],
            'usage' => [
                'inputTokens' => 100,
                'outputTokens' => 50
            ]
        ];

        $result = $this->formatResponse($decodedBody);

        $this->assertEquals(
            'Let me help you with that. ',
            $result['content'],
            'Response should only include text before the first tool use'
        );

        $this->assertCount(
            2,
            $result['toolInvocations'],
            'Response should include both tool invocations'
        );

        $this->assertEquals(
            'view_file',
            $result['toolInvocations'][0]['toolName'],
            'First tool invocation should have correct name'
        );

        $this->assertEquals(
            'view_file',
            $result['toolInvocations'][1]['toolName'],
            'Second tool invocation should have correct name'
        );
    }
}