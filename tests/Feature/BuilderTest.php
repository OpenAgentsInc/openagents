<?php

use App\Models\User;

test('authed user can visit builder page', function () {
    $this->actingAs(User::factory()->create())
        ->get('/builder')
        ->assertStatus(200);
});

test('guest cannot visit builder page', function () {
    $this->get('/builder')
        ->assertRedirect('/login');
});
