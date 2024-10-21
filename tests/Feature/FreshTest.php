<?php

use App\Models\User;
use App\Models\Thread;
use App\Models\Message;

use function Pest\Laravel\get;
use function Pest\Laravel\post;
use function Pest\Laravel\actingAs;

beforeEach(function () {
    $this->user = User::factory()->create();
});

test('fresh page loads correctly', function () {
    $response = actingAs($this->user)->get('/fresh');
    $response->assertStatus(200);
    $response->assertViewIs('fresh');
});

test('clicking chat loads messages in main area', function () {
    $thread = Thread::factory()->create(['user_id' => $this->user->id]);
    Message::factory()->count(3)->create(['thread_id' => $thread->id]);

    $response = actingAs($this->user)
        ->withHeaders(['HX-Request' => 'true'])
        ->get("/chat/{$thread->id}/messages");

    $response->assertStatus(200);
    $response->assertViewIs('partials.chat_messages');
    $response->assertViewHas('messages');
    $thread->messages->each(fn ($message) => $response->assertSee($message->content));
});

test('fresh page shows user threads', function () {
    $threads = Thread::factory()->count(3)->create(['user_id' => $this->user->id]);

    $response = actingAs($this->user)->get('/fresh');

    $response->assertStatus(200);
    $response->assertViewIs('fresh');
    $threads->each(fn ($thread) => $response->assertSee($thread->title));
});

test('sending message adds to thread', function () {
    $thread = Thread::factory()->create(['user_id' => $this->user->id]);

    $response = actingAs($this->user)
        ->withHeaders(['HX-Request' => 'true', 'X-Requested-With' => 'XMLHttpRequest'])
        ->post("/chat/{$thread->id}/send", [
            'content' => 'Test message content'
        ]);

    $response->assertStatus(200);
    $response->assertViewIs('partials.chat_messages');
    $response->assertSee('Test message content');

    $this->assertDatabaseHas('messages', [
        'thread_id' => $thread->id,
        'user_id' => $this->user->id,
        'content' => 'Test message content'
    ]);
});

test('unauthorized user cannot access fresh page', function () {
    $response = get('/fresh');
    $response->assertStatus(302);
    $response->assertRedirect('/login');
});

test('unauthorized user cannot send message', function () {
    $thread = Thread::factory()->create();

    $response = post("/chat/{$thread->id}/send", [
        'content' => 'Test message content'
    ]);

    $response->assertStatus(302);
    $response->assertRedirect('/login');
});

test('user cannot access other users threads', function () {
    $otherUser = User::factory()->create();
    $thread = Thread::factory()->create(['user_id' => $otherUser->id]);

    $response = actingAs($this->user)
        ->withHeaders(['HX-Request' => 'true'])
        ->get("/chat/{$thread->id}/messages");

    $response->assertStatus(403);
});

test('empty message is not sent', function () {
    $thread = Thread::factory()->create(['user_id' => $this->user->id]);

    $response = actingAs($this->user)
        ->withHeaders(['HX-Request' => 'true', 'X-Requested-With' => 'XMLHttpRequest'])
        ->post("/chat/{$thread->id}/send", [
            'content' => ''
        ]);

    $response->assertStatus(422);
    $response->assertJsonValidationErrors(['content']);
});

test('empty message list shows correct message', function () {
    $thread = Thread::factory()->create(['user_id' => $this->user->id]);

    $response = actingAs($this->user)
        ->withHeaders(['HX-Request' => 'true'])
        ->get("/chat/{$thread->id}/messages");

    $response->assertStatus(200);
    $response->assertViewIs('partials.chat_messages');
    $response->assertSee('Send your first message', false);
});