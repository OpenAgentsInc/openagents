<?php

use App\Models\User;
use App\Models\Thread;
use App\Models\Project;
use Illuminate\Foundation\Testing\RefreshDatabase;

uses(RefreshDatabase::class);

test('authenticated user can send a message without a project', function () {
    $user = User::factory()->create();

    $response = $this->actingAs($user)
        ->post('/send-message', [
            'message' => 'Test message'
        ]);

    $response->assertStatus(302);
    $response->assertSessionHas('success', 'Message sent successfully!');

    $this->assertDatabaseHas('messages', [
        'user_id' => $user->id,
        'content' => 'Test message'
    ]);

    $this->assertDatabaseHas('threads', [
        'user_id' => $user->id,
        'title' => 'Test message...'
    ]);
});

test('authenticated user can send a message with a project', function () {
    $user = User::factory()->create();
    $project = Project::factory()->create(['user_id' => $user->id]);

    $response = $this->actingAs($user)
        ->post('/send-message', [
            'message' => 'Test message',
            'project_id' => $project->id
        ]);

    $response->assertStatus(302);
    $response->assertSessionHas('success', 'Message sent successfully!');

    $this->assertDatabaseHas('messages', [
        'user_id' => $user->id,
        'content' => 'Test message'
    ]);

    $this->assertDatabaseHas('threads', [
        'user_id' => $user->id,
        'project_id' => $project->id,
        'title' => 'Test message...'
    ]);
});

test('authenticated user can send a message to an existing thread', function () {
    $user = User::factory()->create();
    $project = Project::factory()->create(['user_id' => $user->id]);
    $thread = Thread::factory()->create(['user_id' => $user->id, 'project_id' => $project->id]);

    $response = $this->actingAs($user)
        ->post('/send-message', [
            'message' => 'Test message',
            'thread_id' => $thread->id
        ]);

    $response->assertStatus(302);
    $response->assertSessionHas('success', 'Message sent successfully!');

    $this->assertDatabaseHas('messages', [
        'user_id' => $user->id,
        'thread_id' => $thread->id,
        'content' => 'Test message'
    ]);
});

test('unauthenticated user cannot send a message', function () {
    $response = $this->post('/send-message', [
        'message' => 'Test message'
    ]);

    $response->assertStatus(302);
    $response->assertRedirect('/login');
});

test('message cannot be empty', function () {
    $user = User::factory()->create();

    $response = $this->actingAs($user)
        ->post('/send-message', [
            'message' => ''
        ]);

    $response->assertStatus(302);
    $response->assertSessionHasErrors('message');
});

test('project_id must be valid if provided', function () {
    $user = User::factory()->create();

    $response = $this->actingAs($user)
        ->post('/send-message', [
            'message' => 'Test message',
            'project_id' => 999 // Non-existent project ID
        ]);

    $response->assertStatus(302);
    $response->assertSessionHasErrors('project_id');
});