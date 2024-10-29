<?php

use App\AI\BedrockMessageConverter;

beforeEach(function () {
    $this->converter = new BedrockMessageConverter();
});

test('bedrock message converter handles empty content correctly', function () {
    $messages = [
        [
            'role' => 'user',
            'content' => 'Open the README on the openagentsinc/openagents main branch and summarize in 1 sentence.'
        ],
        [
            'role' => 'assistant',
            'content' => ' ',  // Empty content with just a space
            'toolInvocations' => [
                [
                    'state' => 'result',
                    'toolCallId' => 'tooluse_4-7M_FtMSIOPt-b_TuezDg',
                    'toolName' => 'view_file',
                    'args' => [
                        'owner' => 'openagentsinc',
                        'repo' => 'openagents',
                        'path' => 'README.md',
                        'branch' => 'main'
                    ],
                    'result' => [
                        'type' => 'tool_call',
                        'value' => [
                            'toolCallId' => 'tooluse_4-7M_FtMSIOPt-b_TuezDg',
                            'toolName' => 'view_file',
                            'args' => [
                                'owner' => 'openagentsinc',
                                'repo' => 'openagents',
                                'path' => 'README.md',
                                'branch' => 'main'
                            ],
                            'result' => [
                                'success' => true,
                                'content' => "Test content",
                                'sha' => 'test-sha',
                                'summary' => 'Test summary',
                                'details' => 'Test details'
                            ]
                        ]
                    ]
                ]
            ]
        ]
    ];

    $result = $this->converter->convertToBedrockChatMessages($messages);

    // Verify that no empty text fields are present in the result
    assertNoEmptyTextFields($result['messages']);

    // Verify the structure of the messages
    expect($result['messages'])->toHaveCount(3)
        ->and($result['messages'][0]['role'])->toBe('user')
        ->and($result['messages'][1]['role'])->toBe('assistant')
        ->and($result['messages'][1]['content'][0])->toHaveKey('toolUse')
        ->and($result['messages'][2]['role'])->toBe('user')
        ->and($result['messages'][2]['content'][0])->toHaveKey('toolResult');
});

test('bedrock message converter removes empty assistant messages', function () {
    $messages = [
        [
            'role' => 'user',
            'content' => 'Test message'
        ],
        [
            'role' => 'assistant',
            'content' => ' ',  // Empty content with just a space
        ],
        [
            'role' => 'user',
            'content' => 'Continue'
        ]
    ];

    $result = $this->converter->convertToBedrockChatMessages($messages);

    // The empty assistant message should be removed, and we should have:
    // 1. User "Test message"
    // 2. Assistant "I understand"
    // 3. User "Continue"
    expect($result['messages'])->toHaveCount(3)
        ->and($result['messages'][0]['role'])->toBe('user')
        ->and($result['messages'][0]['content'][0]['text'])->toBe('Test message')
        ->and($result['messages'][1]['role'])->toBe('assistant')
        ->and($result['messages'][1]['content'][0]['text'])->toBe('I understand.')
        ->and($result['messages'][2]['role'])->toBe('user')
        ->and($result['messages'][2]['content'][0]['text'])->toBe('Continue');
});

// Helper function to recursively check for empty text fields
function assertNoEmptyTextFields(array $messages) {
    foreach ($messages as $message) {
        if (!isset($message['content'])) {
            continue;
        }

        foreach ($message['content'] as $block) {
            if (isset($block['text'])) {
                expect(trim($block['text']))->not()->toBe('');
            }
            if (isset($block['toolResult'])) {
                foreach ($block['toolResult']['content'] as $content) {
                    if (isset($content['text'])) {
                        expect(trim($content['text']))->not()->toBe('');
                    }
                }
            }
        }
    }
}