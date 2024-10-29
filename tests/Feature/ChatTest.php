<?php

use App\Models\Thread;
use App\Models\User;
use Inertia\Testing\AssertableInertia as Assert;

test('chat page shows thread messages', function () {
    $user = User::factory()->create();
    $thread = Thread::factory()->create([
        'user_id' => $user->id,
        'title' => 'Test Chat'
    ]);

    // Create some messages for the thread
    $messages = [
        [
            'role' => 'user',
            'content' => 'Hello',
            'created_at' => now()->toISOString()
        ],
        [
            'role' => 'assistant',
            'content' => 'Hi there!',
            'created_at' => now()->toISOString()
        ]
    ];

    // Update thread with messages
    $thread->messages = $messages;
    $thread->save();

    $response = $this
        ->actingAs($user)
        ->get("/chat/{$thread->id}");

    $response->assertInertia(fn (Assert $page) => $page
        ->component('Chat')
        ->has('thread', fn (Assert $thread) => $thread
            ->where('id', $thread->id)
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