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
