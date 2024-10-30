<?php

use App\Models\Thread;
use App\Models\User;
use App\Models\Message;
use App\Models\ToolInvocation;
use Inertia\Testing\AssertableInertia as Assert;

test('chat page shows thread messages with tool invocations', function () {
    $user = User::factory()->create();
    $thread = Thread::factory()->create([
        'user_id' => $user->id,
        'title' => 'Test Chat'
    ]);

    // Create user message
    Message::create([
        'thread_id' => $thread->id,
        'role' => 'user',
        'content' => 'Hello',
        'created_at' => now()
    ]);

    // Create assistant message with tool invocations
    $assistantMessage = Message::create([
        'thread_id' => $thread->id,
        'role' => 'assistant',
        'content' => 'Hi there!',
        'created_at' => now()
    ]);

    // Add tool invocations to the assistant message
    ToolInvocation::factory()->create([
        'message_id' => $assistantMessage->id,
        'tool_name' => 'view_file',
        'input' => ['path' => 'test/file.txt'],
        'output' => ['content' => 'file contents'],
        'status' => 'completed'
    ]);

    $response = $this
        ->actingAs($user)
        ->get("/chat/{$thread->id}");

    $response->assertInertia(fn (Assert $page) => $page
        ->component('Chat')
        ->has('messages', 2)
        ->has('messages.0', fn (Assert $message) => $message
            ->where('role', 'user')
            ->where('content', 'Hello')
            ->etc()
        )
        ->has('messages.1', fn (Assert $message) => $message
            ->where('role', 'assistant')
            ->where('content', 'Hi there!')
            ->has('toolInvocations')
            ->has('toolInvocations.0', fn (Assert $tool) => $tool
                ->where('tool_name', 'view_file')
                ->where('status', 'completed')
                ->etc()
            )
            ->etc()
        )
        ->where('currentChatId', $thread->id)
    );
});