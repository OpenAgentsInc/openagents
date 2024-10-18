<?php

use App\Models\User;
use App\Models\Thread;
use Illuminate\Foundation\Testing\RefreshDatabase;

uses(RefreshDatabase::class);

test('authenticated user can send a message from homepage and is redirected to new chat thread', function () {
    $user = User::factory()->create();

    $response = $this->actingAs($user)
        ->post('/send-message', [
            'message' => 'Test message from homepage'
        ]);

    $response->assertStatus(302); // Assert that there's a redirect
    
    $thread = Thread::latest()->first();
    $response->assertRedirect("/chat/{$thread->id}");

    $this->assertDatabaseHas('messages', [
        'user_id' => $user->id,
        'content' => 'Test message from homepage'
    ]);

    $this->assertDatabaseHas('threads', [
        'user_id' => $user->id,
        'title' => 'Test message from homepage...'
    ]);

    // Follow the redirect and check the chat page
    $chatResponse = $this->actingAs($user)->get("/chat/{$thread->id}");
    $chatResponse->assertStatus(200);
    $chatResponse->assertSee('Test message from homepage');
    $chatResponse->assertSee($thread->title);
});

test('unauthenticated user is redirected to login when trying to send a message from homepage', function () {
    $response = $this->post('/send-message', [
        'message' => 'Test message from homepage'
    ]);

    $response->assertStatus(302);
    $response->assertRedirect('/login');
});

test('chat page loads correctly after sending a message', function () {
    $user = User::factory()->create();

    $response = $this->actingAs($user)
        ->post('/send-message', [
            'message' => 'Another test message'
        ]);

    $thread = Thread::latest()->first();

    $chatResponse = $this->actingAs($user)->get("/chat/{$thread->id}");
    $chatResponse->assertStatus(200);
    $chatResponse->assertSee('Another test message');
    $chatResponse->assertSee($thread->title);
    $chatResponse->assertSee('Send'); // Assuming there's a "Send" button on the chat page
});