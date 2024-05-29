<?php

use App\Enums\Currency;
use App\Models\Agent;
use App\Models\User;
use App\Services\PaymentService;

test('sats_earned is calculated correctly', function () {
    $agent = Agent::factory()->withBalance(800000, Currency::BTC)->create();
    $user = User::factory()->withBalance(300000, Currency::BTC)->create();
    $this->actingAs($user);

    // First we expect it to be 0 because we only care about payments received
    expect($agent->sats_earned)->toBe(0)
        ->and($user->sats_earned)->toBe(0);

    // Now we have the User make a payment to the Agent
    $payService = new PaymentService();
    $res = $payService->payAgentForMessage($agent->id, 8);

    expect($res)->toBeTrue()
        // Now we expect the Agent to have earned 8 sats
        ->and($agent->fresh()->sats_earned)->toBe(8)
        ->and($user->sats_earned)->toBe(0);
});

test('sats_earned is calculated correctly even after a masspay', function () {
    $agent = Agent::factory()->withBalance(800000, Currency::BTC)->create();
    $user = User::factory()->withBalance(300000, Currency::BTC)->create();

    // run artisan command masspay
    Artisan::call('masspay');

    $this->actingAs($user);

    // First we expect it to be 0 because we only care about payments received
    expect($agent->sats_earned)->toBe(0)
        ->and($user->sats_earned)->toBe(500);

    // Now we have the User make a payment to the Agent
    $payService = new PaymentService();
    $res = $payService->payAgentForMessage($agent->id, 8);

    expect($res)->toBeTrue()
        // Now we expect the Agent to have earned 8 sats
        ->and($agent->fresh()->sats_earned)->toBe(8)
        ->and($user->sats_earned)->toBe(500);
});
