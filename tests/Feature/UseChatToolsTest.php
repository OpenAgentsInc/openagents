<?php

namespace Tests\Feature;

use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;
use Illuminate\Support\Facades\Http;

class UseChatToolsTest extends TestCase
{
    use RefreshDatabase;

    public function test_chat_tools_response_format()
    {
        $user = User::factory()->create();

        $payload = [
            'messages' => [
                [
                    'role' => 'user',
                    'content' => 'Open the README on the openagentsinc/openagents main branch and summarize in 1 sentence.'
                ],
                [
                    'role' => 'assistant',
                    'content' => 'Certainly! I\'ll use the `view_file` function to open the README file from the main branch of the openagentsinc/openagents repository and then summarize it for you in one sentence. Here\'s the function call: ',
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
                                'type' => 'tool_call',
                                'value' => [
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
            ->postJson('/api/chat', $payload);

        $response->assertStatus(200)
            ->assertHeader('Content-Type', 'text/event-stream; charset=UTF-8')
            ->assertHeader('X-Accel-Buffering', 'no')
            ->assertHeader('Cache-Control', 'no-cache');

        // Get the response content
        $content = $response->streamedContent();

        // Verify the response contains the expected event types
        $this->assertStringContainsString('0:', $content); // Text delta
        $this->assertStringContainsString('9:', $content); // Tool call
        $this->assertStringContainsString('a:', $content); // Tool result

        // Verify tool call contains required fields
        $this->assertStringContainsString('toolCallId', $content);
        $this->assertStringContainsString('toolName', $content);
        $this->assertStringContainsString('args', $content);

        // Verify tool result contains required fields
        $this->assertStringContainsString('success', $content);
        $this->assertStringContainsString('error', $content);
        $this->assertStringContainsString('details', $content);
    }
}