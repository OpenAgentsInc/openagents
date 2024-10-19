<?php

namespace Tests\Feature;

use App\Models\User;
use App\Models\Team;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class TeamSwitchTest extends TestCase
{
    use RefreshDatabase;

    public function test_user_can_switch_team()
    {
        // Create a user
        $user = User::factory()->create();

        // Create two teams
        $team1 = Team::factory()->create();
        $team2 = Team::factory()->create();

        // Associate user with both teams
        $user->teams()->attach([$team1->id, $team2->id]);

        // Set the initial current team
        $user->current_team_id = $team1->id;
        $user->save();

        // Attempt to switch to team2
        $response = $this->actingAs($user)->post(route('switch-team', $team2->id));

        // Assert the response is a redirect
        $response->assertRedirect();

        // Assert that the user's current team has been updated
        $this->assertEquals($team2->id, $user->fresh()->current_team_id);
    }

    public function test_user_cannot_switch_to_unauthorized_team()
    {
        // Create a user
        $user = User::factory()->create();

        // Create two teams
        $team1 = Team::factory()->create();
        $team2 = Team::factory()->create();

        // Associate user with only team1
        $user->teams()->attach($team1->id);

        // Set the initial current team
        $user->current_team_id = $team1->id;
        $user->save();

        // Attempt to switch to team2 (which the user doesn't belong to)
        $response = $this->actingAs($user)->post(route('switch-team', $team2->id));

        // Assert the response is a redirect
        $response->assertRedirect();

        // Assert that an error message is flashed to the session
        $response->assertSessionHas('error', 'You do not have access to this team.');

        // Assert that the user's current team has not changed
        $this->assertEquals($team1->id, $user->fresh()->current_team_id);
    }
}