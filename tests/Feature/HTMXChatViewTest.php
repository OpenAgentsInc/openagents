<?php

use App\Models\User;
use App\Models\Thread;
use App\Models\Team;
use Illuminate\Foundation\Testing\RefreshDatabase;

uses(RefreshDatabase::class);

test('clicking a chat updates main content with correct HTML', function () {
    // Create a user, team, and thread
    $user = User::factory()->create();
    $team = Team::factory()->create();
    $user->teams()->attach($team);
    $user->update(['current_team_id' => $team->id]);
    $thread = Thread::factory()->create(['user_id' => $user->id, 'team_id' => $team->id]);

    // Simulate an HTMX request to view a specific chat
    $response = $this->actingAs($user)
        ->withHeaders(['HX-Request' => 'true'])
        ->get(route('chat.show', $thread));

    // Assert the response status is 200 (OK)
    $response->assertStatus(200);

    // Assert that the response contains the expected HTML structure
    $response->assertSee('id="main-content-inner"', false);
    $response->assertSee('id="chat-content"', false);
    $response->assertSee('id="message-list"', false);

    // Assert that the response contains the thread title
    $response->assertSee($thread->title);

    // Assert that the response doesn't contain full HTML structure (it's a partial update)
    $response->assertDontSee('<html');
    $response->assertDontSee('</html>');

    // Assert that the correct view is used
    $response->assertViewIs('chat.show');

    // Assert that the view has the necessary variables
    $response->assertViewHas('thread', $thread);
    $response->assertViewHas('messages');
});

test('sending a message updates the chat content', function () {
    // Create a user, team, and thread
    $user = User::factory()->create();
    $team = Team::factory()->create();
    $user->teams()->attach($team);
    $user->update(['current_team_id' => $team->id]);
    $thread = Thread::factory()->create(['user_id' => $user->id, 'team_id' => $team->id]);

    // Simulate an HTMX request to send a message
    $response = $this->actingAs($user)
        ->withHeaders(['HX-Request' => 'true'])
        ->post(route('chat.send', $thread), [
            'content' => 'Test message'
        ]);

    // Assert the response status is 200 (OK)
    $response->assertStatus(200);

    // Assert that the response contains the new message
    $response->assertSee('Test message');

    // Assert that the correct view is used
    $response->assertViewIs('partials.message');

    // Assert that the view has the necessary variables
    $response->assertViewHas('message');
});