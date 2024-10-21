<?php

use App\Models\User;
use App\Models\Team;
use App\Models\Project;
use App\Models\Thread;
use App\Models\Message;

test('a user belongs to many teams', function () {
    $user = User::factory()->create();
    $teams = Team::factory()->count(2)->create();
    $user->teams()->attach($teams);

    expect($user->teams)->toHaveCount(2);
    expect($user->teams->first())->toBeInstanceOf(Team::class);
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

test('a user can have projects through their teams', function () {
    $user = User::factory()->create();
    $team = Team::factory()->create();
    $user->teams()->attach($team);
    $projects = Project::factory()->count(3)->create(['team_id' => $team->id]);

    expect($user->teams->first()->projects)->toHaveCount(3);
    expect($user->teams->first()->projects->first())->toBeInstanceOf(Project::class);
});

test('a user can create thread, respecting team/project', function () {
    $user = User::factory()->create();
    $team = Team::factory()->create();
    $user->teams()->attach($team);
    $projects = Project::factory()->count(3)->create(['team_id' => $team->id]);

    $user->createThread([
        'title' => 'My first thread',
    ]);

    // TODO: handle every permutation of passing in no team/project, team but no project, project but no team, and both team and project
});
