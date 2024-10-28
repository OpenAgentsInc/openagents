<?php

namespace Tests\Unit;

use App\AI\BedrockMessageConverter;
use PHPUnit\Framework\TestCase;

class BedrockMessageConverterTest extends TestCase
{
    private BedrockMessageConverter $converter;

    protected function setUp(): void
    {
        parent::setUp();
        $this->converter = new BedrockMessageConverter();
    }

    public function test_converts_simple_user_message()
    {
        $messages = [
            [
                'role' => 'user',
                'content' => 'Hello'
            ]
        ];

        $result = $this->converter->convertToBedrockChatMessages($messages);

        $this->assertEquals([
            'system' => null,
            'messages' => [
                [
                    'role' => 'user',
                    'content' => [
                        ['text' => 'Hello']
                    ]
                ]
            ]
        ], $result);
    }

    public function test_converts_user_and_assistant_messages()
    {
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

        $this->assertEquals([
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
        ], $result);
    }

    public function test_converts_system_message()
    {
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

        $this->assertEquals([
            'system' => 'You are a helpful assistant',
            'messages' => [
                [
                    'role' => 'user',
                    'content' => [
                        ['text' => 'Hello']
                    ]
                ]
            ]
        ], $result);
    }

    public function test_throws_exception_for_assistant_first_message()
    {
        $this->expectException(\Exception::class);

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

        $this->converter->convertToBedrockChatMessages($messages);
    }

    public function test_converts_tool_results()
    {
        $messages = [
            [
                'role' => 'user',
                'content' => 'Show me the README'
            ],
            [
                'role' => 'assistant',
                'content' => 'I\'ll help you with that.',
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

        $this->assertEquals([
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
                            'toolResult' => [
                                'toolUseId' => 'tool123',
                                'content' => [
                                    ['text' => json_encode([
                                        'content' => 'README content here'
                                    ])]
                                ]
                            ]
                        ]
                    ]
                ],
                [
                    'role' => 'user',
                    'content' => [
                        ['text' => 'Continue.']
                    ]
                ]
            ]
        ], $result);
    }
}