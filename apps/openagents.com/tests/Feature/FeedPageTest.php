<?php

use App\Models\Shout;
use App\Models\User;

it('renders the feed page for guests', function () {
    $this->get('/feed')
        ->assertOk()
        ->assertInertia(fn ($page) => $page->component('feed'));
});

it('renders the feed page for authenticated users', function () {
    $user = User::factory()->create();

    $this->actingAs($user)
        ->get('/feed')
        ->assertOk()
        ->assertInertia(fn ($page) => $page->component('feed'));
});

it('filters feed by zone and supports all view', function () {
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

    $this->get('/feed?zone=l402')
        ->assertOk()
        ->assertInertia(fn ($page) => $page
            ->component('feed')
            ->where('feed.zone', 'l402')
            ->where('feed.items.0.body', 'L402-only shout body')
            ->where('feed.items', fn ($items) => count($items) === 1)
        );

    $this->get('/feed?zone=all')
        ->assertOk()
        ->assertInertia(fn ($page) => $page
            ->component('feed')
            ->where('feed.zone', null)
            ->where('feed.items', fn ($items) => count($items) === 2)
        );
});
