<?php

use App\Enums\Currency;
use App\Models\User;

test('it has a sats balance', function () {
    $user = User::factory()->withBalance(1030, Currency::BTC)->create(); // msats

    expect($user->sats_balance)->toBe(1);
});
