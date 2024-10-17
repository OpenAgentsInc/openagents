<?php

use App\Models\Thread;
use App\Models\User;
use App\Models\Project;
use App\Models\Message;
use App\Models\Team;

test('a thread belongs to a user', function () {
    $user = User::factory()->create();
    $thread = Thread::factory()->create(['user_id' => $user->id]);

    expect($thread->user)->toBeInstanceOf(User::class);
    expect($thread->user->id)->toBe($user->id);
});

test('a thread belongs to a project', function () {
    $project = Project::factory()->create();
    $thread = Thread::factory()->create(['project_id' => $project->id]);

    expect($thread->project)->toBeInstanceOf(Project::class);
    expect($thread->project->id)->toBe($project->id);
});

test('a thread has many messages', function () {
    $thread = Thread::factory()->create();
    $messages = Message::factory()->count(3)->create(['thread_id' => $thread->id]);

    expect($thread->messages)->toHaveCount(3);
    expect($thread->messages->first())->toBeInstanceOf(Message::class);
});

test('a thread belongs to a team through a project', function () {
    $team = Team::factory()->create();
    $project = Project::factory()->create(['team_id' => $team->id]);
    $thread = Thread::factory()->create(['project_id' => $project->id]);

    expect($thread->project->team)->toBeInstanceOf(Team::class);
    expect($thread->project->team->id)->toBe($team->id);
});