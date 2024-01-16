<?php

use App\Models\User;

test('profile page returns 200 status code and profile view', function () {
    $user = User::factory()->create();

    $response = $this->actingAs($user)->get('/profile');

    $response->assertStatus(200)
    ->assertViewIs('profile');
});

test('profile page redirects to login if user is not logged in', function () {
    $response = $this->get('/profile');

    $response->assertStatus(302)
    ->assertRedirect('/login');
});
