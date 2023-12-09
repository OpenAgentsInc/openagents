<?php

use App\Models\User;

test('authed user can visit the dashboard', function () {
    $user = User::factory()->create();
    $this->actingAs($user);

    $this->get('/dashboard')
        ->assertOk();
});
