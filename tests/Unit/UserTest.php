<?php

use App\Models\User;
use App\Models\Team;
use App\Models\Project;
use App\Models\Thread;
use App\Models\Message;

test('a user can belong to multiple teams', function () {
    $user = User::factory()->create();
    $teams = Team::factory()->count(3)->create();

    $user->teams()->attach($teams->pluck('id'));

    expect($user->teams)->toHaveCount(3);
    expect($user->teams->first())->toBeInstanceOf(Team::class);
});

test('a user can have a current team', function () {
    $user = User::factory()->create();
    $team = Team::factory()->create();

    $user->current_team_id = $team->id;
    $user->save();

    expect($user->currentTeam)->toBeInstanceOf(Team::class);
    expect($user->currentTeam->id)->toBe($team->id);
});

test('a user can have a null current team for personal context', function () {
    $user = User::factory()->create(['current_team_id' => null]);

    expect($user->currentTeam)->toBeNull();
});

test('a user has many projects', function () {
    $user = User::factory()->create();
    $projects = Project::factory()->count(3)->create(['user_id' => $user->id]);

    expect($user->projects)->toHaveCount(3);
    expect($user->projects->first())->toBeInstanceOf(Project::class);
});

test('a user has many threads', function () {
    $user = User::factory()->create();
    $threads = Thread::factory()->count(3)->create(['user_id' => $user->id]);

    expect($user->threads)->toHaveCount(3);
    expect($user->threads->first())->toBeInstanceOf(Thread::class);
});

test('a user has many messages', function () {
    $user = User::factory()->create();
    $messages = Message::factory()->count(3)->create(['user_id' => $user->id]);

    expect($user->messages)->toHaveCount(3);
    expect($user->messages->first())->toBeInstanceOf(Message::class);
});

test('a user can have projects through their current team', function () {
    $team = Team::factory()->create();
    $user = User::factory()->create(['current_team_id' => $team->id]);
    $projects = Project::factory()->count(3)->create(['team_id' => $team->id]);

    expect($user->currentTeam->projects)->toHaveCount(3);
    expect($user->currentTeam->projects->first())->toBeInstanceOf(Project::class);
});