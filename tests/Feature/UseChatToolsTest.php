<?php

use App\Models\User;

test('chat tools response has correct format', function () {
    $this->withoutExceptionHandling();

    $user = User::factory()->create();
    $user->threads()->create(['title' => 'New Chat']);

    $payload = [
        'messages' => [
            [
                'role' => 'user',
                'content' => 'Open the README on the openagentsinc/openagents main branch and summarize in 1 sentence.'
            ],
            [
                'role' => 'assistant',
                'content' => [
                    [
                        'type' => 'text',
                        'text' => 'I\'ll help you with that.'
                    ],
                    [
                        'type' => 'tool-call',
                        'toolCallId' => 'tooluse_HPDKhe_NSMyVyE53Cw_dKQ',
                        'toolName' => 'view_file',
                        'args' => [
                            'owner' => 'openagentsinc',
                            'repo' => 'openagents',
                            'path' => 'README.md',
                            'branch' => 'main'
                        ]
                    ]
                ],
                'toolInvocations' => [
                    [
                        'state' => 'result',
                        'toolCallId' => 'tooluse_HPDKhe_NSMyVyE53Cw_dKQ',
                        'toolName' => 'view_file',
                        'args' => [
                            'owner' => 'openagentsinc',
                            'repo' => 'openagents',
                            'path' => 'README.md',
                            'branch' => 'main'
                        ],
                        'result' => [
                            'success' => false,
                            'error' => 'Failed to retrieve file from GitHub',
                            'details' => 'Client error: `GET https://api.github.com/repos/openagentsinc/openagents/contents/README.md?ref=main` resulted in a `401 Unauthorized` response:\n{"message":"Bad credentials","documentation_url":"https://docs.github.com/rest","status":"401"}\n'
                        ]
                    ]
                ]
            ]
        ],
        'thread_id' => 1,
        'selected_tools' => [
            'view_file',
            'view_folder'
        ]
    ];

    $response = $this->actingAs($user)
        ->postJson('/chat', $payload);

    // Assert response headers
    expect($response->getStatusCode())->toBe(200)
        ->and($response->headers->get('Content-Type'))->toBe('text/event-stream; charset=UTF-8')
        ->and($response->headers->get('X-Accel-Buffering'))->toBe('no')
        ->and($response->headers->get('Cache-Control'))->toBe('no-cache');

    // Get streamed content
    $content = $response->streamedContent();

    // Assert event types exist
    expect($content)
        ->toContain('0:') // Text delta
        ->toContain('9:') // Tool call
        ->toContain('a:'); // Tool result

    // Assert tool call fields
    expect($content)
        ->toContain('toolCallId')
        ->toContain('toolName')
        ->toContain('args');

    // Assert tool result fields
    expect($content)
        ->toContain('success')
        ->toContain('error')
        ->toContain('details');
});