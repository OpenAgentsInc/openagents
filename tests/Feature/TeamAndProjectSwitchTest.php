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

test('user can switch team', function () {
    $response = $this->actingAs($this->user)
        ->post(route('switch-team', $this->team2->id));

    $response->assertStatus(200);
    $response->assertSee('Team 2');
    $response->assertSee('Project 3');
    $response->assertDontSee('Project 1');
    $response->assertDontSee('Project 2');

    $this->user->refresh();
    expect($this->user->current_team_id)->toBe($this->team2->id);
    expect($this->user->current_project_id)->toBeNull();
});

test('user can switch project', function () {
    $response = $this->actingAs($this->user)
        ->post(route('switch-project', $this->project2->id));

    $response->assertStatus(200);
    $response->assertSee('Team 1');
    $response->assertSee('Project 2');

    $this->user->refresh();
    expect($this->user->current_project_id)->toBe($this->project2->id);
});

test('user cannot switch to a team they do not belong to', function () {
    $otherTeam = Team::factory()->create(['name' => 'Other Team']);

    $response = $this->actingAs($this->user)
        ->post(route('switch-team', $otherTeam->id));

    $response->assertStatus(403);
    $this->user->refresh();
    expect($this->user->current_team_id)->toBe($this->team1->id);
});

test('user cannot switch to a project they do not have access to', function () {
    $otherProject = Project::factory()->create(['team_id' => Team::factory()->create()->id, 'name' => 'Other Project']);

    $response = $this->actingAs($this->user)
        ->post(route('switch-project', $otherProject->id));

    $response->assertStatus(403);
    $this->user->refresh();
    expect($this->user->current_project_id)->toBeNull();
});

test('switching teams resets current project', function () {
    $this->user->current_project_id = $this->project1->id;
    $this->user->save();

    $response = $this->actingAs($this->user)
        ->post(route('switch-team', $this->team2->id));

    $response->assertStatus(200);
    $this->user->refresh();
    expect($this->user->current_team_id)->toBe($this->team2->id);
    expect($this->user->current_project_id)->toBeNull();
});