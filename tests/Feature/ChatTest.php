<?php

use App\Models\Thread;
use App\Models\User;
use App\Models\Message;
use Inertia\Testing\AssertableInertia as Assert;

test('chat page shows thread messages', function () {
    $user = User::factory()->create();
    $thread = Thread::factory()->create([
        'user_id' => $user->id,
        'title' => 'Test Chat'
    ]);

    // Create messages through the relationship
    Message::create([
        'thread_id' => $thread->id,
        'role' => 'user',
        'content' => 'Hello',
        'created_at' => now()
    ]);

    Message::create([
        'thread_id' => $thread->id,
        'role' => 'assistant',
        'content' => 'Hi there!',
        'created_at' => now()
    ]);

    // Store thread ID for assertions
    $threadId = $thread->id;

    $response = $this
        ->actingAs($user)
        ->get("/chat/{$threadId}");

    $response->assertInertia(fn (Assert $page) => $page
        ->component('Chat')
        ->has('thread', fn (Assert $thread) => $thread
            ->where('id', $threadId)
            ->where('title', 'Test Chat')
            ->has('messages', 2)
            ->has('messages.0', fn (Assert $message) => $message
                ->where('role', 'user')
                ->where('content', 'Hello')
                ->etc()
            )
            ->has('messages.1', fn (Assert $message) => $message
                ->where('role', 'assistant')
                ->where('content', 'Hi there!')
                ->etc()
            )
        )
    );
});