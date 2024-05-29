<?php

use App\Enums\Currency;
use App\Models\Agent;
use App\Models\User;
use App\Services\PaymentService;

test('user can pay agent for message', function () {
    $agent = Agent::factory()->create(['sats_per_message' => 3]);
    $user = User::factory()->withBalance(100000, Currency::BTC)->create();
    $this->actingAs($user);

    $payService = new PaymentService();
    $res = $payService->payAgentForMessage($agent->id, $agent->sats_per_message);

    // Assert res is ok
    expect($res)->toBeTrue();

    $user->refresh();

    // Assert there is a payment record
    expect($user->sentPayments()->count())->toBe(1);

    expect($user->sats_balance)->toBe(97);
});
