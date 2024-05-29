<?php

use App\Services\PaymentService;

test('agent balances can be distributed to users', function () {

    // Given a bunch of agents with balances (run the PaymentSeeder)
    $this->artisan('db:seed', ['--class' => 'PaymentSeeder']);

    $payService = new PaymentService();
    $payService->sweepAllAgentBalances();

    // Then the balances of the agents should be zero

});
