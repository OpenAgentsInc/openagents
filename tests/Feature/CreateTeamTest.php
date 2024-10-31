<?php

use App\Models\User;
use App\Models\Team;

test('authenticated user can create a new team', function () {
    $user = User::factory()->create();
    
    $response = $this
        ->actingAs($user)
        ->post('/teams', [
            'name' => 'Test Team'
        ]);

    $response->assertRedirect();
    
    $this->assertDatabaseHas('teams', [
        'name' => 'Test Team'
    ]);

    $team = Team::where('name', 'Test Team')->first();
    $this->assertTrue($team->users->contains($user));
    $this->assertEquals($team->id, $user->fresh()->current_team_id);
});

test('team name is required', function () {
    $user = User::factory()->create();
    
    $response = $this
        ->actingAs($user)
        ->post('/teams', [
            'name' => ''
        ]);

    $response->assertSessionHasErrors(['name']);
});

test('team name must be unique for the creating user', function () {
    $user = User::factory()->create();
    $existingTeam = Team::factory()->create(['name' => 'Existing Team']);
    $user->teams()->attach($existingTeam);
    
    $response = $this
        ->actingAs($user)
        ->post('/teams', [
            'name' => 'Existing Team'
        ]);

    $response->assertSessionHasErrors(['name']);
});

test('guest cannot create a team', function () {
    $response = $this->post('/teams', [
        'name' => 'Test Team'
    ]);

    $response->assertRedirect('/login');
    $this->assertDatabaseMissing('teams', [
        'name' => 'Test Team'
    ]);
});

test('user can switch between teams', function () {
    $user = User::factory()->create();
    $team1 = Team::factory()->create(['name' => 'Team 1']);
    $team2 = Team::factory()->create(['name' => 'Team 2']);
    
    $user->teams()->attach([$team1->id, $team2->id]);
    $user->current_team_id = $team1->id;
    $user->save();
    
    $response = $this
        ->actingAs($user)
        ->post('/switch-team', [
            'team_id' => $team2->id
        ]);

    $response->assertRedirect();
    $this->assertEquals($team2->id, $user->fresh()->current_team_id);
});

test('user can switch to personal context', function () {
    $user = User::factory()->create();
    $team = Team::factory()->create(['name' => 'Team 1']);
    
    $user->teams()->attach($team);
    $user->current_team_id = $team->id;
    $user->save();
    
    $response = $this
        ->actingAs($user)
        ->post('/switch-team', [
            'team_id' => null
        ]);

    $response->assertRedirect();
    $this->assertNull($user->fresh()->current_team_id);
});