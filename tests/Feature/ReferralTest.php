<?php

use App\Models\User;
use Inertia\Testing\AssertableInertia as Assert;

test('if referral session var is set, set the referrer on new user signup', function () {
    $referrer = User::factory()->create();

    session()->put('r', $referrer->username);

    $user = User::create([
        'name' => 'John Doe',
        'github_nickname' => 'johndoe',
        'email' => 'flamp@flamp.com',
    ]);

    $this->assertEquals($referrer->id, $user->referrer_id);
    $this->assertEquals($referrer->username, $user->referrer->username);
});

test('if visiting a referral link, set referral session variable', function () {
    $referrer = User::factory()->create();

    $this->get('/?r=' . $referrer->username);

    $this->assertEquals($referrer->username, session('r'));
});

test('referrals page knows user referrals', function () {
    $user = User::factory()->create();
    $referral = User::factory()->create(['referrer_id' => $user->id]);

    $this->actingAs($user)
        ->get('referrals')
        ->assertInertia(
            fn (Assert $page) => $page
            ->component('Referrals')
            ->has('referrals')
            ->where('referrals.0.id', $referral->id)
            ->where('referrals.0.username', $referral->username)
        );
});
