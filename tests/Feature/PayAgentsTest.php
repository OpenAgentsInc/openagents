<?php

use App\Enums\Currency;
use App\Models\Agent;
use App\Models\User;
use App\Services\PaymentService;

test('user can pay agent for message', function () {
    $user = User::factory()->withBalance(100000, Currency::BTC)->create();
    $agent = Agent::factory()->create(['sats_per_message' => 3]);

    $payService = new PaymentService();
    $res = $payService->payAgentForMessage($agent->id, $agent->sats_per_message);

    // Assert res is ok
    expect($res)->toBeTrue();

    $user->refresh();

    // Assert there is a payment record
    expect($user->payments()->count())->toBe(1);

    expect($user->sats_balance)->toBe(97);
});
