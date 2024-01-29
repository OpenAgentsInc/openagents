<?php

namespace Tests\Feature;

use App\Models\User;
use App\Models\Withdrawal;

test('authed user can visit withdraw page', function () {
    $user = User::factory()->create();
    $this->actingAs($user)
        ->get(route('withdraw'))
        ->assertStatus(200);
});

test('unauthed user is redirected to login page', function () {
    $this->get(route('withdraw'))
        ->assertRedirect(route('login'));
})->skip();

test('user must have lightning_address to withdraw', function () {
    $user = User::factory()->create(['lightning_address' => null]);
    $this->actingAs($user)
        ->post(route('withdraw'), [
            'amount' => 10, // sats
        ])
        ->assertSee('You must set a Lightning address before withdrawing.');
});

test('user can complete withdrawal', function () {
    // expect database to have no Withdrawal records
    expect(Withdrawal::count())->toBe(0);

    $user = User::factory()->create(['lightning_address' => 'atlantispleb@getalby.com']);
    $this->actingAs($user)
        ->post(route('withdraw'), [
            'amount' => 10, // sats
        ]);

    expect(Withdrawal::count())->toBe(1);
    expect(Withdrawal::first()->status)->toBe('completed');
})->group('integration');
