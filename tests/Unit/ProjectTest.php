<?php

use App\Models\Project;
use App\Models\Thread;
use App\Models\User;
use App\Models\Team;

test('a project belongs to a user', function () {
    $user = User::factory()->create();
    $project = Project::factory()->create(['user_id' => $user->id]);

    expect($project->user)->toBeInstanceOf(User::class);
    expect($project->user->id)->toBe($user->id);
});

test('a project belongs to a team', function () {
    $team = Team::factory()->create();
    $project = Project::factory()->create(['team_id' => $team->id]);

    expect($project->team)->toBeInstanceOf(Team::class);
    expect($project->team->id)->toBe($team->id);
});

test('a project has many threads', function () {
    $project = Project::factory()->create();
    $threads = Thread::factory()->count(3)->create(['project_id' => $project->id]);

    expect($project->threads)->toHaveCount(3);
    expect($project->threads->first())->toBeInstanceOf(Thread::class);
});

test('a project belongs to either a user or a team', function () {
    $userProject = Project::factory()->create(['user_id' => User::factory()]);
    $teamProject = Project::factory()->create(['team_id' => Team::factory()]);

    expect($userProject->user)->toBeInstanceOf(User::class);
    expect($userProject->team)->toBeNull();

    expect($teamProject->team)->toBeInstanceOf(Team::class);
    expect($teamProject->user)->toBeNull();
});