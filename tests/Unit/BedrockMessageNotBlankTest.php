<?php

use App\AI\BedrockMessageConverter;

beforeEach(function () {
    $this->converter = new BedrockMessageConverter();
});

test('bedrock message is not blank', function () {
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

    dd($result);

    expect($result)->not()->toBe([
        'system' => null,
        'messages' => []
    ]);
});
