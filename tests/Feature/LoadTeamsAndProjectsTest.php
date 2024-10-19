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

    public function test_getTeamsAndProjects_returns_actual_teams_and_projects()
    {
        // Create a user
        $user = User::factory()->create();

        // Create teams
        $team1 = Team::factory()->create();
        $team2 = Team::factory()->create();

        // Associate user with teams
        $user->teams()->attach([$team1->id, $team2->id]);

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
        $response->assertSee($project3->name);

        // Assert that the response doesn't contain teams not associated with the user
        $team3 = Team::factory()->create();
        $response->assertDontSee($team3->name);

        // Assert that the response doesn't contain projects from teams not associated with the user
        $project4 = Project::factory()->create(['team_id' => $team3->id]);
        $response->assertDontSee($project4->name);
    }
}