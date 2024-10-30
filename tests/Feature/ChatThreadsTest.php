<?php

use App\Models\Thread;
use App\Models\User;
use Inertia\Testing\AssertableInertia as Assert;

test('chat page shows user threads', function () {
    $user = User::factory()->create();
    
    // Create three threads for the user
    $threads = Thread::factory()->count(3)->create([
        'user_id' => $user->id
    ]);

    // Create a thread for another user (should not be visible)
    $otherUser = User::factory()->create();
    Thread::factory()->create([
        'user_id' => $otherUser->id
    ]);

    $response = $this
        ->actingAs($user)
        ->get("/chat/{$threads[0]->id}");

    $response->assertInertia(fn (Assert $page) => $page
        ->component('Chat')
        ->has('threads', 3)
        ->has('threads.0', fn (Assert $thread) => $thread
            ->where('id', $threads[0]->id)
            ->where('user_id', $user->id)
            ->where('title', $threads[0]->title)
            ->etc()
        )
    );
});