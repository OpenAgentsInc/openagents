<?php

use App\Models\Team;
use App\Models\User;
use App\Models\Thread;
use App\Models\Project;

test('a team has many users', function () {
    $team = Team::factory()->create();
    $users = User::factory()->count(3)->create(['team_id' => $team->id]);

    expect($team->users)->toHaveCount(3);
    expect($team->users->first())->toBeInstanceOf(User::class);
});

test('a team has many threads', function () {
    $team = Team::factory()->create();
    $threads = Thread::factory()->count(3)->create(['team_id' => $team->id]);

    expect($team->threads)->toHaveCount(3);
    expect($team->threads->first())->toBeInstanceOf(Thread::class);
});

test('a team has many projects', function () {
    $team = Team::factory()->create();
    $projects = Project::factory()->count(3)->create(['team_id' => $team->id]);

    expect($team->projects)->toHaveCount(3);
    expect($team->projects->first())->toBeInstanceOf(Project::class);
});