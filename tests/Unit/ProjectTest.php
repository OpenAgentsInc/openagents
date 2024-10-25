<?php

use App\Models\Project;
use App\Models\Thread;
use App\Models\User;
use App\Models\Team;

test('a project belongs to a user', function () {
    $project = Project::factory()->forUser()->create();

    expect($project->user)->toBeInstanceOf(User::class);
    expect($project->team)->toBeNull();
});

test('a project belongs to a team', function () {
    $project = Project::factory()->forTeam()->create();

    expect($project->team)->toBeInstanceOf(Team::class);
    expect($project->user)->toBeNull();
});

test('a project has many threads', function () {
    $project = Project::factory()->create();
    $threads = Thread::factory()->count(3)->create(['project_id' => $project->id]);

    expect($project->threads)->toHaveCount(3);
    expect($project->threads->first())->toBeInstanceOf(Thread::class);
});

test('a project belongs to either a user or a team', function () {
    $userProject = Project::factory()->forUser()->create();
    $teamProject = Project::factory()->forTeam()->create();

    expect($userProject->user)->toBeInstanceOf(User::class);
    expect($userProject->team)->toBeNull();

    expect($teamProject->team)->toBeInstanceOf(Team::class);
    expect($teamProject->user)->toBeNull();
});