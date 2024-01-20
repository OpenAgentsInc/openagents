<?php

use App\Models\User;

test('profile page returns 200 status code and profile view', function () {
    $user = User::factory()->create();

    $response = $this->actingAs($user)->get('/profile');

    $response->assertStatus(200)
        ->assertViewIs('profile');
})->skip();

test('profile page redirects to login if user is not logged in', function () {
    $response = $this->get('/profile');

    $response->assertStatus(302)
        ->assertRedirect('/login');
})->skip();

test('authed user can update their profile and see success message', function () {
    $user = User::factory()->create();

    $response = $this->actingAs($user)->post('/update-profile', [
        'name' => 'John Doe',
        'email' => 'blah@blah.com',
        'password' => 'password',
        'password_confirmation' => 'password',
    ]);

    $response->assertStatus(200)
        ->assertSee('Profile updated successfully.');
})->skip();
