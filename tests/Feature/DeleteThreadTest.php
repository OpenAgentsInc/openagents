<?php

use App\Models\Thread;
use App\Models\User;

test('user can delete their own thread', function () {
    $user = User::factory()->create();
    $thread = Thread::factory()->create(['user_id' => $user->id]);

    $response = $this->actingAs($user)
        ->delete("/chat/{$thread->id}");

    $response->assertRedirect('/chat');
    $this->assertDatabaseMissing('threads', ['id' => $thread->id]);
});

test('user cannot delete another users thread', function () {
    $user1 = User::factory()->create();
    $user2 = User::factory()->create();
    $thread = Thread::factory()->create(['user_id' => $user2->id]);

    $response = $this->actingAs($user1)
        ->delete("/chat/{$thread->id}");

    $response->assertForbidden();
    $this->assertDatabaseHas('threads', ['id' => $thread->id]);
});

test('deleting thread removes all associated messages', function () {
    $user = User::factory()->create();
    $thread = Thread::factory()->create(['user_id' => $user->id]);
    
    // Create some messages for the thread
    $thread->messages()->create([
        'user_id' => $user->id,
        'content' => 'Test message',
        'role' => 'user'
    ]);

    $this->actingAs($user)
        ->delete("/chat/{$thread->id}");

    $this->assertDatabaseMissing('threads', ['id' => $thread->id]);
    $this->assertDatabaseMissing('messages', ['thread_id' => $thread->id]);
});