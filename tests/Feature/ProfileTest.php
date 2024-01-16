<?php

use App\Models\User;

test('profile page returns 200 status code and profile view', function () {
    $user = User::factory()->create();

    $response = $this->actingAs($user)->get('/profile');

    $response->assertStatus(200)
    ->assertViewIs('profile');
});
