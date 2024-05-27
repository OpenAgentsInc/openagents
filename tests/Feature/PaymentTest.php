<?php

use App\Enums\Currency;
use App\Models\Balance;
use App\Models\Payment;
use App\Models\User;

test('user can pay an agent', function () {
    $user = User::factory()->create();
    $agent = User::factory()->create();
    $balance = Balance::factory()->create([
        'holder_type' => get_class($user),
        'holder_id' => $user->id,
        'currency' => Currency::USD,
        'amount' => 1000,
    ]);

    $payment = Payment::create([
        'payer_type' => get_class($user),
        'payer_id' => $user->id,
        'payee_type' => get_class($agent),
        'payee_id' => $agent->id,
        'currency' => Currency::USD,
        'amount' => 100,
    ]);

    $balance->refresh();
    expect($balance->amount)->toBe(900)
        ->and($payment->amount)->toBe(100);
});
