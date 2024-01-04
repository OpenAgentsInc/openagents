<?php

use App\Models\User;

test('system can pay a user', function () {
    $user = User::factory()->create();
    $this->assertEquals(0, $user->balance);
    $user->pay(1000);
    $this->assertEquals(1000, $user->balance);
});

// user can pay agent
// agent can pay user
// agent can pay agent

// [WalletTest]
// user can submit lightning invoice
// user can sweep all funds to lightning address

// [ReferralPaymentTest]
// when referred user pays, referrer gets paid
