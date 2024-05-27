<?php

use App\Enums\Currency;
use App\Models\Agent;
use App\Models\User;

test('user can pay an agent', function () {
    $user = User::factory()->withBalance(1000 * 1000, Currency::BTC)->create();
    $agent = Agent::factory()->withBalance(0, Currency::BTC)->create();

    $user->payAgent($agent, 1000 * 1000, Currency::BTC);

    expect($user->checkBalance(Currency::BTC))->toBe(0)
        ->and($agent->checkBalance(Currency::BTC))->toBe(1000 * 1000);
});

test('user can pay a user', function () {
    $user = User::factory()->withBalance(1000 * 1000, Currency::BTC)->create();
    $user2 = User::factory()->withBalance(0, Currency::BTC)->create();

    $user->payUser($user2, 1000 * 1000, Currency::BTC);

    expect($user->checkBalance(Currency::BTC))->toBe(0)
        ->and($user2->checkBalance(Currency::BTC))->toBe(1000 * 1000);
});

test('agent can pay a user', function () {
    $agent = Agent::factory()->withBalance(1000 * 1000, Currency::BTC)->create();
    $user = User::factory()->withBalance(0, Currency::BTC)->create();

    $agent->payUser($user, 1000 * 1000, Currency::BTC);

    expect($agent->checkBalance(Currency::BTC))->toBe(0)
        ->and($user->checkBalance(Currency::BTC))->toBe(1000 * 1000);
});

test('agent can pay an agent', function () {
    $agent = Agent::factory()->withBalance(1000 * 1000, Currency::BTC)->create();
    $agent2 = Agent::factory()->withBalance(0, Currency::BTC)->create();

    $agent->payAgent($agent2, 1000 * 1000, Currency::BTC);

    expect($agent->checkBalance(Currency::BTC))->toBe(0)
        ->and($agent2->checkBalance(Currency::BTC))->toBe(1000 * 1000);
});
