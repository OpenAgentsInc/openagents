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
    $project = Project::factory()->create(['team_id' => $team->id]);

    // Test case 1: No team/project
    $thread1 = $user->createThread([
        'title' => 'Thread without team or project',
    ]);
    expect($thread1)->toBeInstanceOf(Thread::class);
    expect($thread1->team_id)->toBeNull();
    expect($thread1->project_id)->toBeNull();

    // Test case 2: Team but no project
    $thread2 = $user->createThread([
        'title' => 'Thread with team only',
        'team_id' => $team->id,
    ]);
    expect($thread2)->toBeInstanceOf(Thread::class);
    expect($thread2->team_id)->toBe($team->id);
    expect($thread2->project_id)->toBeNull();

    // Test case 3: Project but no team
    $thread3 = $user->createThread([
        'title' => 'Thread with project only',
        'project_id' => $project->id,
    ]);
    expect($thread3)->toBeInstanceOf(Thread::class);
    expect($thread3->team_id)->toBe($team->id); // The team should be inferred from the project
    expect($thread3->project_id)->toBe($project->id);

    // Test case 4: Both team and project
    $thread4 = $user->createThread([
        'title' => 'Thread with team and project',
        'team_id' => $team->id,
        'project_id' => $project->id,
    ]);
    expect($thread4)->toBeInstanceOf(Thread::class);
    expect($thread4->team_id)->toBe($team->id);
    expect($thread4->project_id)->toBe($project->id);

    // Test case 5: Mismatched team and project
    $anotherTeam = Team::factory()->create();
    $user->teams()->attach($anotherTeam);
    $anotherProject = Project::factory()->create(['team_id' => $anotherTeam->id]);

    expect(fn () => $user->createThread([
        'title' => 'Thread with mismatched team and project',
        'team_id' => $team->id,
        'project_id' => $anotherProject->id,
    ]))->toThrow(\InvalidArgumentException::class, 'The provided project does not belong to the specified team.');

    // Test case 6: No title provided
    $thread6 = $user->createThread([]);
    expect($thread6)->toBeInstanceOf(Thread::class);
    expect($thread6->title)->toBe('New chat');
});