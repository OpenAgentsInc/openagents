<?php

use App\Models\Thread;
use App\Models\User;
use App\Models\Message;

test('chat messages are saved to specified thread', function () {
    $user = User::factory()->create();
    
    // Create two threads for the user
    $thread1 = Thread::factory()->create([
        'user_id' => $user->id,
        'title' => 'Thread 1'
    ]);
    
    $thread2 = Thread::factory()->create([
        'user_id' => $user->id,
        'title' => 'Thread 2'
    ]);

    // Send a message specifying thread_id = 2
    $response = $this->actingAs($user)
        ->postJson('/chat', [
            'messages' => [
                [
                    'role' => 'user',
                    'content' => 'Hello from thread 2'
                ]
            ],
            'thread_id' => $thread2->id,
            'selected_tools' => []
        ]);

    $response->assertStatus(200);

    // Check that message was saved to thread 2, not thread 1 or hardcoded thread_id 1
    $this->assertDatabaseHas('messages', [
        'thread_id' => $thread2->id,
        'content' => json_encode([['text' => 'Hello from thread 2']]),
        'role' => 'user'
    ]);

    $this->assertDatabaseMissing('messages', [
        'thread_id' => $thread1->id,
        'content' => json_encode([['text' => 'Hello from thread 2']]),
    ]);

    $this->assertDatabaseMissing('messages', [
        'thread_id' => 1, // Should not use hardcoded thread_id 1
        'content' => json_encode([['text' => 'Hello from thread 2']]),
    ]);
});