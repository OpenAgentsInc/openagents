<?php

use App\Models\Team;
use App\Models\Thread;
use App\Models\User;
use App\Models\Project;

test('threads are created without project when no team is selected', function () {
    $user = User::factory()->create();
    
    $this->actingAs($user)
        ->post(route('chat.create'));

    $thread = Thread::latest()->first();
    
    expect($thread->user_id)->toBe($user->id)
        ->and($thread->project_id)->toBeNull();
});

test('threads are created with default project when team is selected', function () {
    $user = User::factory()->create();
    $team = Team::factory()->create();
    $user->teams()->attach($team);
    $user->current_team_id = $team->id;
    $user->save();
    
    $this->actingAs($user)
        ->post(route('chat.create'));

    $thread = Thread::latest()->first();
    $project = Project::where('team_id', $team->id)
        ->where('is_default', true)
        ->first();
    
    expect($thread->user_id)->toBe($user->id)
        ->and($thread->project_id)->toBe($project->id)
        ->and($project->team_id)->toBe($team->id);
});

test('user can view their personal threads', function () {
    $user = User::factory()->create();
    $personalThread = Thread::factory()->create([
        'user_id' => $user->id,
        'project_id' => null
    ]);
    
    $this->actingAs($user)
        ->get(route('chat.id', $personalThread->id))
        ->assertStatus(200);
});

test('user can view their team threads', function () {
    $user = User::factory()->create();
    $team = Team::factory()->create();
    $user->teams()->attach($team);
    $project = Project::factory()->create(['team_id' => $team->id]);
    $teamThread = Thread::factory()->create([
        'user_id' => $user->id,
        'project_id' => $project->id
    ]);
    
    $this->actingAs($user)
        ->get(route('chat.id', $teamThread->id))
        ->assertStatus(200);
});

test('user cannot view threads from teams they dont belong to', function () {
    $user = User::factory()->create();
    $otherTeam = Team::factory()->create();
    $project = Project::factory()->create(['team_id' => $otherTeam->id]);
    $teamThread = Thread::factory()->create([
        'project_id' => $project->id
    ]);
    
    $this->actingAs($user)
        ->get(route('chat.id', $teamThread->id))
        ->assertStatus(403);
});

test('user cannot view other users personal threads', function () {
    $user = User::factory()->create();
    $otherUser = User::factory()->create();
    $otherUserThread = Thread::factory()->create([
        'user_id' => $otherUser->id,
        'project_id' => null
    ]);
    
    $this->actingAs($user)
        ->get(route('chat.id', $otherUserThread->id))
        ->assertStatus(403);
});

test('threads list shows only personal threads in personal context', function () {
    $user = User::factory()->create();
    $personalThread = Thread::factory()->create([
        'user_id' => $user->id,
        'project_id' => null
    ]);
    
    $team = Team::factory()->create();
    $project = Project::factory()->create(['team_id' => $team->id]);
    $teamThread = Thread::factory()->create([
        'user_id' => $user->id,
        'project_id' => $project->id
    ]);
    
    $this->actingAs($user)
        ->get(route('chat'))
        ->assertRedirect(route('chat.id', $personalThread->id));
});

test('threads list shows only team threads in team context', function () {
    $user = User::factory()->create();
    $team = Team::factory()->create();
    $user->teams()->attach($team);
    $user->current_team_id = $team->id;
    $user->save();
    
    $personalThread = Thread::factory()->create([
        'user_id' => $user->id,
        'project_id' => null
    ]);
    
    $project = Project::factory()->create(['team_id' => $team->id]);
    $teamThread = Thread::factory()->create([
        'user_id' => $user->id,
        'project_id' => $project->id
    ]);
    
    $this->actingAs($user)
        ->get(route('chat'))
        ->assertRedirect(route('chat.id', $teamThread->id));
});