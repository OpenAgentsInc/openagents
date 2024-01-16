<?php

use App\Models\User;

test('profile page returns 200 status code', function () {
    $user = User::factory()->create();

    $response = $this->actingAs($user)->get('/profile');

    $response->assertStatus(200);
});
