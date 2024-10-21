<?php

use App\Models\User;
use App\Models\Team;
use App\Models\Project;
use App\Models\Thread;
use Illuminate\Foundation\Testing\RefreshDatabase;

uses(RefreshDatabase::class);

beforeEach(function () {
    $this->user = User::factory()->create();
    $this->team = Team::factory()->create();
    $this->project = Project::factory()->create(['team_id' => $this->team->id]);
    $this->user->teams()->attach($this->team);
});

test('unauthenticated user cannot fetch chats', function () {
    $response = $this->get('/api/chats');
    $response->assertStatus(401);
});

test('authenticated user can fetch chats for a team', function () {
    $threads = Thread::factory()->count(3)->create([
        'project_id' => $this->project->id,
        'user_id' => $this->user->id,
    ]);

    $response = $this->actingAs($this->user)
        ->get("/api/chats?team_id={$this->team->id}");

    $response->assertStatus(200);
    $response->assertViewIs('partials.chat-list');
    $response->assertViewHas('threads', function ($viewThreads) use ($threads) {
        return $viewThreads->count() === 3 &&
               $viewThreads->pluck('id')->diff($threads->pluck('id'))->isEmpty();
    });
});

test('authenticated user can fetch chats for a specific project', function () {
    $projectThreads = Thread::factory()->count(2)->create([
        'project_id' => $this->project->id,
        'user_id' => $this->user->id,
    ]);
    
    $otherProject = Project::factory()->create(['team_id' => $this->team->id]);
    Thread::factory()->create([
        'project_id' => $otherProject->id,
        'user_id' => $this->user->id,
    ]);

    $response = $this->actingAs($this->user)
        ->get("/api/chats?team_id={$this->team->id}&project_id={$this->project->id}");

    $response->assertStatus(200);
    $response->assertViewIs('partials.chat-list');
    $response->assertViewHas('threads', function ($viewThreads) use ($projectThreads) {
        return $viewThreads->count() === 2 &&
               $viewThreads->pluck('id')->diff($projectThreads->pluck('id'))->isEmpty();
    });
});

test('authenticated user cannot fetch chats for a team they do not belong to', function () {
    $otherTeam = Team::factory()->create();

    $response = $this->actingAs($this->user)
        ->get("/api/chats?team_id={$otherTeam->id}");

    $response->assertStatus(403);
});

test('chat list is paginated', function () {
    Thread::factory()->count(25)->create([
        'project_id' => $this->project->id,
        'user_id' => $this->user->id,
    ]);

    $response = $this->actingAs($this->user)
        ->get("/api/chats?team_id={$this->team->id}");

    $response->assertStatus(200);
    $response->assertViewIs('partials.chat-list');
    $response->assertViewHas('threads', function ($viewThreads) {
        return $viewThreads->count() === 15; // Assuming default pagination is 15 items per page
    });
});

test('chat list can be sorted by latest message', function () {
    $oldThread = Thread::factory()->create([
        'project_id' => $this->project->id,
        'user_id' => $this->user->id,
        'updated_at' => now()->subDays(2),
    ]);
    
    $newThread = Thread::factory()->create([
        'project_id' => $this->project->id,
        'user_id' => $this->user->id,
        'updated_at' => now(),
    ]);

    $response = $this->actingAs($this->user)
        ->get("/api/chats?team_id={$this->team->id}&sort=latest");

    $response->assertStatus(200);
    $response->assertViewIs('partials.chat-list');
    $response->assertViewHas('threads', function ($viewThreads) use ($newThread, $oldThread) {
        return $viewThreads->first()->id === $newThread->id &&
               $viewThreads->last()->id === $oldThread->id;
    });
});