<?php

use App\Models\User;

test('guests are redirected to the login page for chat', function () {
    $this->get('/chat')->assertRedirect('/login');
});

test('authenticated users can visit chat', function () {
    $this->actingAs(User::factory()->create());

    $this->get('/chat')->assertRedirect();
});
