<?php

use App\Models\User;
use App\Models\Thread;
use App\Models\Team;
use App\Models\Project;
use Illuminate\Foundation\Testing\RefreshDatabase;

uses(RefreshDatabase::class);

test('creating a new thread updates sidebar and main content', function () {
    $user = User::factory()->create();
    $team = Team::factory()->create();
    $project = Project::factory()->create(['team_id' => $team->id]);
    $user->teams()->attach($team);
    $user->update(['current_team_id' => $team->id]);
    $user->update(['current_project_id' => $project->id]);

    $response = $this->actingAs($user)
        ->withHeaders(['HX-Request' => 'true'])
        ->post(route('threads.create'));

    $response->assertStatus(200);
    $response->assertJsonStructure([
        'threadList',
        'chatContent',
        'url'
    ]);

    $responseData = $response->json();

    // Check that the thread list in the sidebar is updated
    $this->assertStringContainsString('New Chat', $responseData['threadList']);

    // Check that the main content area is updated with the new chat
    $this->assertStringContainsString('id="chat-content"', $responseData['chatContent']);
    $this->assertStringContainsString('id="message-list"', $responseData['chatContent']);

    // Check that the URL is updated
    $newThread = Thread::latest()->first();
    $this->assertEquals(route('threads.show', $newThread->id), $responseData['url']);

    // Check that the HX-Push-Url header is set
    $response->assertHeader('HX-Push-Url', route('threads.show', $newThread->id));
});

test('selecting a thread updates main content without full page reload', function () {
    $user = User::factory()->create();
    $thread = Thread::factory()->create(['user_id' => $user->id]);

    $response = $this->actingAs($user)
        ->withHeaders(['HX-Request' => 'true'])
        ->get(route('threads.show', $thread->id));

    $response->assertStatus(200);
    $response->assertViewIs('threads.show');
    $response->assertViewHas('thread', $thread);
    $response->assertViewHas('messages');

    // Check that only the main content is returned, not a full page
    $response->assertDontSee('<html');
    $response->assertDontSee('</html>');
});

test('switching projects updates thread list in sidebar', function () {
    $user = User::factory()->create();
    $team = Team::factory()->create();
    $project1 = Project::factory()->create(['team_id' => $team->id]);
    $project2 = Project::factory()->create(['team_id' => $team->id]);
    $user->teams()->attach($team);
    $user->update(['current_team_id' => $team->id]);

    Thread::factory()->create(['user_id' => $user->id, 'project_id' => $project1->id, 'title' => 'Project 1 Thread']);
    Thread::factory()->create(['user_id' => $user->id, 'project_id' => $project2->id, 'title' => 'Project 2 Thread']);

    $response = $this->actingAs($user)
        ->withHeaders(['HX-Request' => 'true'])
        ->get(route('threads.index', ['project_id' => $project2->id]));

    $response->assertStatus(200);
    $response->assertViewIs('partials.thread-list');
    $response->assertSee('Project 2 Thread');
    $response->assertDontSee('Project 1 Thread');
});
