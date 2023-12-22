<?php

use App\Models\User;
use Inertia\Testing\AssertableInertia as Assert;

test('stats page knows number of users', function () {
    $user = User::factory(2)->create();
    $this->get('stats')
        ->assertInertia(
            fn (Assert $page) => $page
            ->component('Stats')
            ->has('userCount')
            ->where('userCount', User::count())
            ->where('userCount', 2)
        );
});
