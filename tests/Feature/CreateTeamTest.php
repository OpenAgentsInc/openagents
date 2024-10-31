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

test('newly created team becomes users current team', function () {
    $user = User::factory()->create();
    $oldTeam = Team::factory()->create();
    $user->teams()->attach($oldTeam);
    $user->current_team_id = $oldTeam->id;
    $user->save();
    
    $response = $this
        ->actingAs($user)
        ->post('/teams', [
            'name' => 'New Team'
        ]);

    $newTeam = Team::where('name', 'New Team')->first();
    $this->assertEquals($newTeam->id, $user->fresh()->current_team_id);
});