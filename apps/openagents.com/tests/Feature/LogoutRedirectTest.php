<?php

use App\Models\User;

use function Pest\Laravel\actingAs;
use function Pest\Laravel\post;

beforeEach(function () {
    config()->set('app.url', 'https://openagents.com.test');
});

test('logout redirects to app root url and clears auth session', function () {
    $user = User::factory()->create();

    actingAs($user);

    post('/logout')
        ->assertRedirect('https://openagents.com.test');

    $this->assertGuest();
});
