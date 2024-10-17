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

    $response = $this->actingAs($user)->post('/messages', [
        'thread_id' => $thread->id,
        'content' => 'Test message',
    ]);

    $response->assertStatus(201);
    $this->assertDatabaseHas('messages', [
        'thread_id' => $thread->id,
        'user_id' => $user->id,
        'content' => 'Test message',
    ]);
});

test('system can add a message to a thread', function () {
    $thread = Thread::factory()->create();

    $response = $this->post('/system-messages', [
        'thread_id' => $thread->id,
        'content' => 'System response',
    ]);

    $response->assertStatus(201);
    $this->assertDatabaseHas('messages', [
        'thread_id' => $thread->id,
        'user_id' => null,
        'content' => 'System response',
    ]);
});

test('threads can be organized into projects', function () {
    $project = Project::factory()->create();
    $thread = Thread::factory()->create(['project_id' => $project->id]);

    $response = $this->get("/projects/{$project->id}/threads");

    $response->assertStatus(200);
    $response->assertJson([$thread->toArray()]);
});

test('threads can be organized into teams', function () {
    $team = Team::factory()->create();
    $thread = Thread::factory()->create(['team_id' => $team->id]);

    $response = $this->get("/teams/{$team->id}/threads");

    $response->assertStatus(200);
    $response->assertJson([$thread->toArray()]);
});

test('system can make LLM tool calls with GitHub API', function () {
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

    $response = $this->post("/threads/{$thread->id}/process", [
        'message_id' => $message->id,
    ]);

    $response->assertStatus(200);
    $response->assertJson(['success' => true]);
});