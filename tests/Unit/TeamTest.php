<?php

use App\Models\Team;
use App\Models\User;
use App\Models\Thread;
use App\Models\Project;

test('a team can have many users', function () {
    $team = Team::factory()->create();
    $users = User::factory()->count(3)->create();

    $team->users()->attach($users->pluck('id'));

    expect($team->users)->toHaveCount(3);
    expect($team->users->first())->toBeInstanceOf(User::class);
});

test('a user can be a member of multiple teams', function () {
    $teams = Team::factory()->count(3)->create();
    $user = User::factory()->create();

    $user->teams()->attach($teams->pluck('id'));

    expect($user->teams)->toHaveCount(3);
    expect($user->teams->first())->toBeInstanceOf(Team::class);
});

test('a team has many projects', function () {
    $team = Team::factory()->create();
    $projects = Project::factory()->count(3)->create(['team_id' => $team->id]);

    expect($team->projects)->toHaveCount(3);
    expect($team->projects->first())->toBeInstanceOf(Project::class);
});

test('a team has many threads through projects', function () {
    $team = Team::factory()->create();
    $project = Project::factory()->create(['team_id' => $team->id]);
    $threads = Thread::factory()->count(3)->create(['project_id' => $project->id]);

    expect($team->threads)->toHaveCount(3);
    expect($team->threads->first())->toBeInstanceOf(Thread::class);
});

test('a user can have a current team', function () {
    $team = Team::factory()->create();
    $user = User::factory()->create(['current_team_id' => $team->id]);

    expect($user->currentTeam)->toBeInstanceOf(Team::class);
    expect($user->currentTeam->id)->toBe($team->id);
});

test('a team can have many users with it as their current team', function () {
    $team = Team::factory()->create();
    $users = User::factory()->count(3)->create(['current_team_id' => $team->id]);

    expect($team->currentUsers)->toHaveCount(3);
    expect($team->currentUsers->first())->toBeInstanceOf(User::class);
});