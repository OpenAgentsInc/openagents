<?php

use App\Models\User;

test('profile shows username', function () {
    $this->withoutExceptionHandling();

    User::factory()->create([
        'username' => 'username',
        'auth_provider' => 'X',
        'external_id' => 'username',
        'name' => 'name',
    ]);

    $response = $this->get('/u/username');

    $response->assertStatus(200)
        ->assertSee('username');
});
