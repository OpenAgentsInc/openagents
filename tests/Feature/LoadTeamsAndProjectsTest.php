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
        $team1 = Team::factory()->create();
        $team2 = Team::factory()->create();

        // Associate user with teams
        $user->teams()->attach([$team1->id, $team2->id]);

        // Set active team
        $user->currentTeam = $team1;
        $user->save();

        // Create projects
        $project1 = Project::factory()->create(['team_id' => $team1->id]);
        $project2 = Project::factory()->create(['team_id' => $team1->id]);
        $project3 = Project::factory()->create(['team_id' => $team2->id]);

        // Make request to getTeamsAndProjects endpoint
        $response = $this->actingAs($user)->get('/teams');

        // Assert response status
        $response->assertStatus(200);

        // Assert that the response contains the correct teams and projects
        $response->assertSee($team1->name);
        $response->assertSee($team2->name);
        $response->assertSee($project1->name);
        $response->assertSee($project2->name);
        $response->assertDontSee($project3->name);

        // Assert that the response doesn't contain teams not associated with the user
        $team3 = Team::factory()->create();
        $response->assertDontSee($team3->name);

        // Assert that the response doesn't contain projects from teams not associated with the user
        $project4 = Project::factory()->create(['team_id' => $team3->id]);
        $response->assertDontSee($project4->name);
    }

    public function test_getTeamsAndProjects_returns_personal_projects_when_no_active_team()
    {
        // Create a user
        $user = User::factory()->create();

        // Create teams
        $team1 = Team::factory()->create();

        // Associate user with teams
        $user->teams()->attach([$team1->id]);

        // Create projects
        $personalProject = Project::factory()->create(['team_id' => null, 'user_id' => $user->id]);
        $teamProject = Project::factory()->create(['team_id' => $team1->id]);

        // Make request to getTeamsAndProjects endpoint
        $response = $this->actingAs($user)->get('/teams');

        // Assert response status
        $response->assertStatus(200);

        // Assert that the response contains the correct teams and projects
        $response->assertSee($team1->name);
        $response->assertSee($personalProject->name);
        $response->assertDontSee($teamProject->name);
    }
}