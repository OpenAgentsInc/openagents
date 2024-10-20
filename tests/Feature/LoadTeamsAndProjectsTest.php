<?php

use App\Models\User;
use App\Models\Team;
use App\Models\Project;
use Illuminate\Foundation\Testing\RefreshDatabase;

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
});

test('initial page load does not contain teams and projects', function () {
    $response = $this->actingAs($this->user)->get(route('dashboard'));

    $response->assertStatus(200);
    $response->assertDontSee('Team 1');
    $response->assertDontSee('Team 2');
    $response->assertDontSee('Project 1');
    $response->assertDontSee('Project 2');
    $response->assertDontSee('Project 3');
});

test('HTMX endpoint returns teams and projects for active team', function () {
    $response = $this->actingAs($this->user)->get(route('teams.get'));

    $response->assertStatus(200);
    $response->assertSee('Team 1');
    $response->assertSee('Team 2');
    $response->assertSee('Project 1');
    $response->assertSee('Project 2');
    $response->assertDontSee('Project 3');
});

test('HTMX endpoint does not return teams not associated with the user', function () {
    $team3 = Team::factory()->create(['name' => 'Team 3']);
    $project4 = Project::factory()->create(['team_id' => $team3->id, 'name' => 'Project 4']);

    $response = $this->actingAs($this->user)->get(route('teams.get'));

    $response->assertStatus(200);
    $response->assertDontSee('Team 3');
    $response->assertDontSee('Project 4');
});

test('HTMX endpoint returns personal projects when no active team', function () {
    $this->user->current_team_id = null;
    $this->user->save();

    $personalProject = Project::factory()->create(['team_id' => null, 'user_id' => $this->user->id, 'name' => 'Personal Project']);

    $response = $this->actingAs($this->user)->get(route('teams.get'));

    $response->assertStatus(200);
    $response->assertSee('Team 1');
    $response->assertSee('Personal Project');
    $response->assertDontSee('Project 1');
    $response->assertDontSee('Project 2');
});

test('switching teams updates the active team and projects', function () {
    $response = $this->actingAs($this->user)
        ->post(route('switch-team', $this->team2->id));

    $response->assertStatus(200);
    $response->assertSee('Team 2');
    $response->assertSee('Project 3');
    $response->assertDontSee('Project 1');
    $response->assertDontSee('Project 2');
});