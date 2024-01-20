<?php

use App\Models\User;
use Inertia\Testing\AssertableInertia as Assert;

test('stats page knows number of users and balances', function () {
    $user = User::factory(2)->create(['balance' => 1000]);
    $this->get('stats')
        ->assertInertia(
            fn (Assert $page) => $page
            ->component('Stats')
            ->has('userCount')
            ->where('userCount', User::count())
            ->where('userCount', 2)
            ->has('userBalanceSum')
            ->where('userBalanceSum', User::sum('balance'))
            ->where('userBalanceSum', 2000)
        );
})->skip();
