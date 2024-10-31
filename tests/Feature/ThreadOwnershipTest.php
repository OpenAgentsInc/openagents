<?php

use App\Models\Team;
use App\Models\Thread;
use App\Models\User;

test('threads are created with personal ownership when no team is selected', function () {
    $user = User::factory()->create();
    
    $this->actingAs($user)
        ->post(route('chat.create'));

    $thread = Thread::latest()->first();
    
    expect($thread->user_id)->toBe($user->id)
        ->and($thread->team_id)->toBeNull();
});

test('threads are created with team ownership when team is selected', function () {
    $user = User::factory()->create();
    $team = Team::factory()->create();
    $user->teams()->attach($team);
    $user->current_team_id = $team->id;
    $user->save();
    
    $this->actingAs($user)
        ->post(route('chat.create'));

    $thread = Thread::latest()->first();
    
    expect($thread->team_id)->toBe($team->id)
        ->and($thread->user_id)->toBe($user->id);
});

test('user can view their personal threads', function () {
    $user = User::factory()->create();
    $personalThread = Thread::factory()->create([
        'user_id' => $user->id,
        'team_id' => null
    ]);
    
    $this->actingAs($user)
        ->get(route('chat.id', $personalThread->id))
        ->assertStatus(200);
});

test('user can view their team threads', function () {
    $user = User::factory()->create();
    $team = Team::factory()->create();
    $user->teams()->attach($team);
    $user->current_team_id = $team->id;
    $user->save();
    
    $teamThread = Thread::factory()->create([
        'user_id' => $user->id,
        'team_id' => $team->id
    ]);
    
    $this->actingAs($user)
        ->get(route('chat.id', $teamThread->id))
        ->assertStatus(200);
});

test('user cannot view threads from teams they dont belong to', function () {
    $user = User::factory()->create();
    $otherTeam = Team::factory()->create();
    $teamThread = Thread::factory()->create([
        'team_id' => $otherTeam->id
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
        'team_id' => null
    ]);
    
    $this->actingAs($user)
        ->get(route('chat.id', $otherUserThread->id))
        ->assertStatus(403);
});

test('threads list shows only personal threads in personal context', function () {
    $user = User::factory()->create();
    $personalThread = Thread::factory()->create([
        'user_id' => $user->id,
        'team_id' => null
    ]);
    $team = Team::factory()->create();
    $teamThread = Thread::factory()->create([
        'user_id' => $user->id,
        'team_id' => $team->id
    ]);
    
    $response = $this->actingAs($user)->get(route('chat'));
    
    expect($response->baseResponse->original['threads'])
        ->toHaveCount(1)
        ->first()->id->toBe($personalThread->id);
});

test('threads list shows only team threads in team context', function () {
    $user = User::factory()->create();
    $team = Team::factory()->create();
    $user->teams()->attach($team);
    $user->current_team_id = $team->id;
    $user->save();
    
    $personalThread = Thread::factory()->create([
        'user_id' => $user->id,
        'team_id' => null
    ]);
    $teamThread = Thread::factory()->create([
        'user_id' => $user->id,
        'team_id' => $team->id
    ]);
    
    $response = $this->actingAs($user)->get(route('chat'));
    
    expect($response->baseResponse->original['threads'])
        ->toHaveCount(1)
        ->first()->id->toBe($teamThread->id);
});