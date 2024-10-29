<?php

use App\AI\BedrockMessageConverter;

beforeEach(function () {
    $this->converter = new BedrockMessageConverter();
});

/**
 * We'll edit the below to reflect this:

{
    "messages": [
        {
            "role": "user",
            "content": "Open the README on the openagentsinc/openagents main branch and summarize in 1 sentence."
        },
        {
            "role": "assistant",
            "content": " ",
            "toolInvocations": [
                {
                    "state": "result",
                    "toolCallId": "tooluse_4-7M_FtMSIOPt-b_TuezDg",
                    "toolName": "view_file",
                    "args": {
                        "owner": "openagentsinc",
                        "repo": "openagents",
                        "path": "README.md",
                        "branch": "main"
                    },
                    "result": {
                        "type": "tool_call",
                        "value": {
                            "toolCallId": "tooluse_4-7M_FtMSIOPt-b_TuezDg",
                            "toolName": "view_file",
                            "args": {
                                "owner": "openagentsinc",
                                "repo": "openagents",
                                "path": "README.md",
                                "branch": "main"
                            },
                            "result": {
                                "success": true,
                                "content": "# OpenAgents v3\n\nOpenAgents is a platform for building, selling and using AI agents.\n\nWe are building v3 from scratch here.\n\n## Tech Stack\n\n- Laravel\n- React\n- Inertia\n- Tailwind\n\n## Previous work\n\n- OpenAgents v1 (Laravel/Livewire) has moved to the [v1 branch](https://github.com/OpenAgentsInc/openagents/tree/v1).\n- OpenAgents v2 (NextJS) has its own repo [here](https://github.com/OpenAgentsInc/v2)\n\n## Resources\n\n- [Follow us on X](https://x.com/OpenAgentsInc)\n- [Wiki](https://github.com/OpenAgentsInc/openagents/wiki)\n- [Stacker News Community](https://stacker.news/~openagents)\n\n## Video series\n\nWe've chronicled most of the development of this platform over multiple months and 100+ videos on X.\n\nSee [episode one](https://twitter.com/OpenAgentsInc/status/1721942435125715086) or\nthe [full episode list](https://github.com/OpenAgentsInc/openagents/wiki/Video-Series).\n",
                                "sha": "b5efe0a6ba5fda0bdeaf5d7f79f21d78e88aca10",
                                "summary": "Viewed file README.md in openagentsinc/openagents on branch main",
                                "details": "File contents have been retrieved from GitHub"
                            }
                        }
                    }
                }
            ]
        }
    ],
    "thread_id": 1,
    "selected_tools": [
        "view_file",
        "view_folder"
    ]
}

 */

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

    expect($result)->not()->toBe([
        'system' => null,
        'messages' => []
    ]);
});
