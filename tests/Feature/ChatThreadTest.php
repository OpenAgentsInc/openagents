<?php

use App\Models\Thread;
use App\Models\User;
use App\Models\Message;
use Illuminate\Support\Facades\Route;
use Inertia\Testing\AssertableInertia as Assert;

test('chat messages are saved to the correct thread', function () {
    // Create two users with their own threads
    $user1 = User::factory()->create();
    $thread1 = Thread::factory()->create([
        'user_id' => $user1->id,
        'title' => 'User 1 Chat'
    ]);

    $user2 = User::factory()->create();
    $thread2 = Thread::factory()->create([
        'user_id' => $user2->id,
        'title' => 'User 2 Chat'
    ]);

    // Create a message in thread 1
    $response1 = $this
        ->actingAs($user1)
        ->post('/api/chat/stream', [
            'messages' => [
                [
                    'role' => 'user',
                    'content' => 'Hello from thread 1'
                ]
            ]
        ]);

    // Create a message in thread 2
    $response2 = $this
        ->actingAs($user2)
        ->post('/api/chat/stream', [
            'messages' => [
                [
                    'role' => 'user',
                    'content' => 'Hello from thread 2'
                ]
            ]
        ]);

    // Check that messages were saved to their respective threads
    $this->assertDatabaseHas('messages', [
        'thread_id' => $thread1->id,
        'content' => json_encode([['text' => 'Hello from thread 1']]),
        'user_id' => $user1->id
    ]);

    $this->assertDatabaseHas('messages', [
        'thread_id' => $thread2->id,
        'content' => json_encode([['text' => 'Hello from thread 2']]),
        'user_id' => $user2->id
    ]);

    // Verify that each thread only shows its own messages
    $response = $this
        ->actingAs($user1)
        ->get("/chat/{$thread1->id}");

    $response->assertInertia(fn (Assert $page) => $page
        ->component('Chat')
        ->has('messages', fn ($messages) => $messages->every(
            fn ($message) => $message['thread_id'] === $thread1->id
        ))
    );

    $response = $this
        ->actingAs($user2)
        ->get("/chat/{$thread2->id}");

    $response->assertInertia(fn (Assert $page) => $page
        ->component('Chat')
        ->has('messages', fn ($messages) => $messages->every(
            fn ($message) => $message['thread_id'] === $thread2->id
        ))
    );
});

test('chat stream endpoint requires thread_id parameter', function () {
    $user = User::factory()->create();
    
    $response = $this
        ->actingAs($user)
        ->post('/api/chat/stream', [
            'messages' => [
                [
                    'role' => 'user',
                    'content' => 'Hello'
                ]
            ]
        ]);

    $response->assertStatus(422);
});

test('chat stream saves messages to specified thread', function () {
    $user = User::factory()->create();
    $thread = Thread::factory()->create([
        'user_id' => $user->id,
        'title' => 'Test Chat'
    ]);

    $response = $this
        ->actingAs($user)
        ->post('/api/chat/stream', [
            'thread_id' => $thread->id,
            'messages' => [
                [
                    'role' => 'user',
                    'content' => 'Hello'
                ]
            ]
        ]);

    $response->assertStatus(200);

    $this->assertDatabaseHas('messages', [
        'thread_id' => $thread->id,
        'content' => json_encode([['text' => 'Hello']]),
        'user_id' => $user->id
    ]);
});