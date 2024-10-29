<?php

use App\AI\BedrockMessageConverter;

beforeEach(function () {
    $this->converter = new BedrockMessageConverter();
});

test('bedrock message converts tool calls and results correctly', function () {
    $messages = [
        [
            'role' => 'user',
            'content' => 'Open the README on the openagentsinc/openagents main branch and summarize in 1 sentence.'
        ],
        [
            'role' => 'assistant',
            'content' => ' ',
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
                                'content' => "# OpenAgents v3\n\nOpenAgents is a platform for building, selling and using AI agents.",
                                "sha" => "b5efe0a6ba5fda0bdeaf5d7f79f21d78e88aca10",
                                "summary" => "Viewed file README.md in openagentsinc/openagents on branch main",
                                "details" => "File contents have been retrieved from GitHub"
                            ]
                        ]
                    ]
                ]
            ]
        ]
    ];

    $result = $this->converter->convertToBedrockChatMessages($messages);

    // Assert structure matches Bedrock's converse API requirements
    expect($result)->toHaveKey('messages');
    expect($result['messages'])->toBeArray();
    
    // Check user message
    expect($result['messages'][0])->toMatchArray([
        'role' => 'user',
        'content' => [
            ['text' => 'Open the README on the openagentsinc/openagents main branch and summarize in 1 sentence.']
        ]
    ]);

    // Check assistant message with tool use
    expect($result['messages'][1])->toMatchArray([
        'role' => 'assistant',
        'content' => [
            [
                'toolUse' => [
                    'toolUseId' => 'tooluse_4-7M_FtMSIOPt-b_TuezDg',
                    'name' => 'view_file',
                    'input' => [
                        'owner' => 'openagentsinc',
                        'repo' => 'openagents',
                        'path' => 'README.md',
                        'branch' => 'main'
                    ]
                ]
            ]
        ]
    ]);

    // Check tool result message
    expect($result['messages'][2])->toMatchArray([
        'role' => 'user',
        'content' => [
            [
                'toolResult' => [
                    'toolUseId' => 'tooluse_4-7M_FtMSIOPt-b_TuezDg',
                    'status' => 'success',
                    'content' => [
                        [
                            'text' => json_encode([
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
                                        'content' => "# OpenAgents v3\n\nOpenAgents is a platform for building, selling and using AI agents.",
                                        'sha' => 'b5efe0a6ba5fda0bdeaf5d7f79f21d78e88aca10',
                                        'summary' => 'Viewed file README.md in openagentsinc/openagents on branch main',
                                        'details' => 'File contents have been retrieved from GitHub'
                                    ]
                                ]
                            ])
                        ]
                    ]
                ]
            ]
        ]
    ]);
});