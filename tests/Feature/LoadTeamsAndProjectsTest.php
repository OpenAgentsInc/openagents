<?php

namespace Tests\Feature;

use App\Models\User;
use App\Models\Team;
use App\Models\Project;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class LoadTeamsAndProjectsTest extends TestCase
{
    use RefreshDatabase;

    public function test_getTeamsAndProjects_returns_teams_and_projects_for_active_team()
    {
        // Create a user
        $user = User::factory()->create();

        // Create teams
        $team1 = Team::factory()->create(['name' => 'Team 1']);
        $team2 = Team::factory()->create(['name' => 'Team 2']);

        // Associate user with teams
        $user->teams()->attach([$team1->id, $team2->id]);

        // Set active team
        $user->current_team_id = $team1->id;
        $user->save();

        // Create projects
        $project1 = Project::factory()->create(['team_id' => $team1->id, 'name' => 'Project 1']);
        $project2 = Project::factory()->create(['team_id' => $team1->id, 'name' => 'Project 2']);
        $project3 = Project::factory()->create(['team_id' => $team2->id, 'name' => 'Project 3']);

        // Make request to getTeamsAndProjects endpoint
        $response = $this->actingAs($user)->get(route('teams.get'));

        // Assert response status
        $response->assertStatus(200);

        // Assert that the response contains the correct teams and projects
        $response->assertSee('Team 1');
        $response->assertSee('Team 2');
        $response->assertSee('Project 1');
        $response->assertSee('Project 2');
        $response->assertDontSee('Project 3');

        // Assert that the response doesn't contain teams not associated with the user
        $team3 = Team::factory()->create(['name' => 'Team 3']);
        $response->assertDontSee('Team 3');

        // Assert that the response doesn't contain projects from teams not associated with the user
        $project4 = Project::factory()->create(['team_id' => $team3->id, 'name' => 'Project 4']);
        $response->assertDontSee('Project 4');
    }

    public function test_getTeamsAndProjects_returns_personal_projects_when_no_active_team()
    {
        // Create a user
        $user = User::factory()->create();

        // Create teams
        $team1 = Team::factory()->create(['name' => 'Team 1']);

        // Associate user with teams
        $user->teams()->attach([$team1->id]);

        // Create projects
        $personalProject = Project::factory()->create(['team_id' => null, 'user_id' => $user->id, 'name' => 'Personal Project']);
        $teamProject = Project::factory()->create(['team_id' => $team1->id, 'name' => 'Team Project']);

        // Make request to getTeamsAndProjects endpoint
        $response = $this->actingAs($user)->get(route('teams.get'));

        // Assert response status
        $response->assertStatus(200);

        // Assert that the response contains the correct teams and projects
        $response->assertSee('Team 1');
        $response->assertSee('Personal Project');
        $response->assertDontSee('Team Project');
    }
}