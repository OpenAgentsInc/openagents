<?php

use App\Models\Shout;
use App\Models\User;

it('redirects guests away from the feed page', function () {
    $this->get('/feed')->assertRedirect('/login');
});

it('renders the feed page for authenticated users', function () {
    $user = User::factory()->create();

    $this->actingAs($user)
        ->get('/feed')
        ->assertOk();
});

it('filters feed by zone and supports all view', function () {
    $viewer = User::factory()->create();
    $author = User::factory()->create();

    Shout::query()->create([
        'user_id' => $author->id,
        'zone' => 'l402',
        'body' => 'L402-only shout body',
        'visibility' => 'public',
    ]);

    Shout::query()->create([
        'user_id' => $author->id,
        'zone' => 'dev',
        'body' => 'Dev-only shout body',
        'visibility' => 'public',
    ]);

    $this->actingAs($viewer)
        ->get('/feed?zone=l402')
        ->assertOk()
        ->assertSee('L402-only shout body')
        ->assertDontSee('Dev-only shout body');

    $this->actingAs($viewer)
        ->get('/feed?zone=all')
        ->assertOk()
        ->assertSee('L402-only shout body')
        ->assertSee('Dev-only shout body');
});
