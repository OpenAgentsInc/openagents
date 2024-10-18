<?php

use App\Models\User;
use App\Models\Message;
use App\Models\Thread;
use App\Models\Project;
use App\Models\Team;
use App\Services\LlmService;
use App\Services\GitHubApiService;

test('user can send a message in a thread', function () {
    $user = User::factory()->create();
    $thread = Thread::factory()->create();

    $response = $this->actingAs($user)->post('/send-message', [
        'thread_id' => $thread->id,
        'message' => 'Test message',
    ]);

    $response->assertStatus(302);
    $response->assertRedirect("/chat/{$thread->id}");
    $this->assertDatabaseHas('messages', [
        'thread_id' => $thread->id,
        'user_id' => $user->id,
        'content' => 'Test message',
    ]);
});

test('system can add a message to a thread', function () {
    $user = User::factory()->create();
    $thread = Thread::factory()->create();

    $response = $this->actingAs($user)->post("/threads/{$thread->id}/messages", [
        'content' => 'System response',
        'user_id' => null,
    ]);

    $response->assertStatus(201);
    $this->assertDatabaseHas('messages', [
        'thread_id' => $thread->id,
        'user_id' => null,
        'content' => 'System response',
        'is_system_message' => true,
    ]);
});

test('threads can be organized into projects', function () {
    $user = User::factory()->create();
    $project = Project::factory()->create();
    $thread = Thread::factory()->create(['project_id' => $project->id]);

    $response = $this->actingAs($user)->get("/projects/{$project->id}/threads");

    $response->assertStatus(200);
    $response->assertJsonFragment($thread->toArray());
});

test('threads can be organized into teams', function () {
    $user = User::factory()->create();
    $team = Team::factory()->create();
    $project = Project::factory()->create(['team_id' => $team->id]);
    $thread = Thread::factory()->create(['project_id' => $project->id]);

    $response = $this->actingAs($user)->get("/teams/{$team->id}/threads");

    $response->assertStatus(200);
    $response->assertJsonFragment($thread->toArray());
});

test('system can make LLM tool calls with GitHub API', function () {
    $user = User::factory()->create();
    $thread = Thread::factory()->create();
    $message = Message::factory()->create([
        'thread_id' => $thread->id,
        'content' => 'Create a new file in the repository',
    ]);

    // Mock the LLM service and GitHub API calls
    $this->mock(LlmService::class, function ($mock) {
        $mock->shouldReceive('processMessage')->andReturn([
            'action' => 'create_file',
            'params' => [
                'path' => 'test.txt',
                'content' => 'Test content',
            ],
        ]);
    });

    $this->mock(GitHubApiService::class, function ($mock) {
        $mock->shouldReceive('createFile')->andReturn(true);
    });

    $response = $this->actingAs($user)->post("/threads/{$thread->id}/process", [
        'message_id' => $message->id,
    ]);

    $response->assertStatus(200);
    $response->assertJson(['success' => true]);
});