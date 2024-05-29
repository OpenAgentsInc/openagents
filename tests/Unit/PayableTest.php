<?php

use App\Enums\Currency;
use App\Models\Agent;

test('sats_earned is calculated correctly', function () {
    $agent = Agent::factory()->withBalance(800000, Currency::BTC)->create();
    $user = Agent::factory()->withBalance(300000, Currency::BTC)->create();

    expect($agent->sats_earned)->toBe(800)
        ->and($user->sats_earned)->toBe(300);
});
