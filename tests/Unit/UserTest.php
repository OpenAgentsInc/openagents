<?php

use App\Models\User;
use App\Models\Team;
use App\Models\Project;
use App\Models\Thread;
use App\Models\Message;

test('a user belongs to a team', function () {
    $team = Team::factory()->create();
    $user = User::factory()->create(['team_id' => $team->id]);

    expect($user->team)->toBeInstanceOf(Team::class);
    expect($user->team->id)->toBe($team->id);
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

test('a user can have projects through their team', function () {
    $team = Team::factory()->create();
    $user = User::factory()->create(['team_id' => $team->id]);
    $projects = Project::factory()->count(3)->create(['team_id' => $team->id]);

    expect($user->team->projects)->toHaveCount(3);
    expect($user->team->projects->first())->toBeInstanceOf(Project::class);
});