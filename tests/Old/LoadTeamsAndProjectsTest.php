<?php

use App\Models\User;
use App\Models\Team;
use App\Models\Project;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Support\Facades\Log;

uses(RefreshDatabase::class);

beforeEach(function () {
    $this->user = User::factory()->create();
    $this->team1 = Team::factory()->create(['name' => 'Team 1']);
    $this->team2 = Team::factory()->create(['name' => 'Team 2']);
    $this->user->teams()->attach([$this->team1->id, $this->team2->id]);
    $this->user->current_team_id = $this->team1->id;
    $this->user->save();

    $this->project1 = Project::factory()->create(['team_id' => $this->team1->id, 'name' => 'Project 1']);
    $this->project2 = Project::factory()->create(['team_id' => $this->team1->id, 'name' => 'Project 2']);
    $this->project3 = Project::factory()->create(['team_id' => $this->team2->id, 'name' => 'Project 3']);

    Log::info('Test setup complete', [
        'user_id' => $this->user->id,
        'team1_id' => $this->team1->id,
        'team2_id' => $this->team2->id,
        'project1_id' => $this->project1->id,
        'project2_id' => $this->project2->id,
        'project3_id' => $this->project3->id,
    ]);
});

test('initial page load does not contain teams and projects', function () {
    $response = $this->actingAs($this->user)->get(route('dashboard'));

    $response->assertStatus(200);
    $response->assertDontSee('Team 1');
    $response->assertDontSee('Team 2');
    $response->assertDontSee('Project 1');
    $response->assertDontSee('Project 2');
    $response->assertDontSee('Project 3');
    $response->assertSee('id="teams-and-projects"');
});

test('HTMX endpoint returns teams and projects for active team', function () {
    $response = $this->actingAs($this->user)->get(route('teams.projects'));

    $response->assertStatus(200);
    
    Log::info('Response content:', ['content' => $response->getContent()]);
    
    $responseContent = $response->getContent();
    
    $this->assertTrue(str_contains($responseContent, 'Team 1'), 'Response does not contain Team 1');
    $this->assertTrue(str_contains($responseContent, 'Team 2'), 'Response does not contain Team 2');
    $this->assertTrue(str_contains($responseContent, 'id="teamSwitcher"'), 'Response does not contain team switcher');
    $this->assertTrue(str_contains($responseContent, 'id="projectSwitcher"'), 'Response does not contain project switcher');
    $this->assertTrue(str_contains($responseContent, 'Project 1'), 'Response does not contain Project 1');
    $this->assertTrue(str_contains($responseContent, 'Project 2'), 'Response does not contain Project 2');
    $this->assertFalse(str_contains($responseContent, 'Project 3'), 'Response contains Project 3 when it should not');

    // Log the projects associated with the active team
    $activeTeamProjects = Project::where('team_id', $this->user->current_team_id)->get();
    Log::info('Active team projects:', $activeTeamProjects->toArray());
});

test('HTMX endpoint does not return teams and projects not associated with the user', function () {
    $team3 = Team::factory()->create(['name' => 'Team 3']);
    $project4 = Project::factory()->create(['team_id' => $team3->id, 'name' => 'Project 4']);

    $response = $this->actingAs($this->user)->get(route('teams.projects'));

    $response->assertStatus(200);
    $response->assertDontSee('Team 3');
    $response->assertDontSee('Project 4');
});

test('HTMX endpoint returns teams and personal projects when no active team', function () {
    $this->user->current_team_id = null;
    $this->user->save();

    $personalProject = Project::factory()->create(['team_id' => null, 'user_id' => $this->user->id, 'name' => 'Personal Project']);

    $response = $this->actingAs($this->user)->get(route('teams.projects'));

    $response->assertStatus(200);
    
    Log::info('Response content for personal projects:', ['content' => $response->getContent()]);
    
    $responseContent = $response->getContent();
    
    $this->assertTrue(str_contains($responseContent, 'Personal'), 'Response does not contain Personal');
    $this->assertTrue(str_contains($responseContent, 'Team 1'), 'Response does not contain Team 1');
    $this->assertTrue(str_contains($responseContent, 'Team 2'), 'Response does not contain Team 2');
    $this->assertTrue(str_contains($responseContent, 'Personal Project'), 'Response does not contain Personal Project');
    $this->assertFalse(str_contains($responseContent, 'Project 1'), 'Response contains Project 1 when it should not');
    $this->assertFalse(str_contains($responseContent, 'Project 2'), 'Response contains Project 2 when it should not');

    // Log the personal projects
    $personalProjects = Project::where('team_id', null)->where('user_id', $this->user->id)->get();
    Log::info('Personal projects:', $personalProjects->toArray());
});

test('switching teams updates the active team and projects', function () {
    $response = $this->actingAs($this->user)
        ->post(route('switch-team', $this->team2->id));

    $response->assertStatus(200);
    
    Log::info('Response content after switching teams:', ['content' => $response->getContent()]);
    
    $responseContent = $response->getContent();
    
    $this->assertTrue(str_contains($responseContent, 'Team 2'), 'Response does not contain Team 2');
    $this->assertTrue(str_contains($responseContent, 'Project 3'), 'Response does not contain Project 3');
    $this->assertFalse(str_contains($responseContent, 'Project 1'), 'Response contains Project 1 when it should not');
    $this->assertFalse(str_contains($responseContent, 'Project 2'), 'Response contains Project 2 when it should not');

    // Log the projects associated with the new active team
    $newActiveTeamProjects = Project::where('team_id', $this->team2->id)->get();
    Log::info('New active team projects:', $newActiveTeamProjects->toArray());
});

test('switching projects updates the active project', function () {
    $response = $this->actingAs($this->user)
        ->post(route('switch-project', $this->project2->id));

    $response->assertStatus(200);
    
    Log::info('Response content after switching projects:', ['content' => $response->getContent()]);
    
    $responseContent = $response->getContent();
    
    $this->assertTrue(str_contains($responseContent, 'Project 2'), 'Response does not contain Project 2');
    $this->assertTrue(str_contains($responseContent, 'Team 1'), 'Response does not contain Team 1');
    $this->assertFalse(str_contains($responseContent, 'Project 3'), 'Response contains Project 3 when it should not');

    // Verify that the active project has been updated
    $this->user->refresh();
    $this->assertEquals($this->project2->id, $this->user->current_project_id);
});