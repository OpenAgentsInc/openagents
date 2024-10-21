<?php

use App\Models\User;

test('homepage loads dashboard view for unauthenticated users', function () {
    $response = $this->get('/');

    $response->assertStatus(200);
    $response->assertViewIs('homepage');
});

test('homepage loads dashboard view for authenticated users', function () {
    $user = User::factory()->create();

    $response = $this->actingAs($user)->get('/');

    $response->assertStatus(200);
    $response->assertViewIs('components.dashboard.dashboard');
});
