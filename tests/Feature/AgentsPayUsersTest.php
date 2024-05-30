<?php

use App\Enums\Currency;
use App\Models\Agent;
use App\Models\Balance;
use App\Services\PaymentService;

test('agent balances can be distributed to users', function () {
    // Given a bunch of agents with balances (run the PaymentSeeder)
    Agent::factory(15)->create()->each(function ($agent) {
        Balance::factory()->create([
            'holder_type' => Agent::class,
            'holder_id' => $agent->id,
            'currency' => Currency::BTC,
            'amount' => 100000,
        ]);
    });

    // Check the total balance of the first 10 agents
    $totalBalance = Agent::all()->sum(function ($agent) {
        return $agent->getSatsBalanceAttribute();
    });

    $this->assertEquals(1500, $totalBalance);

    $payService = new PaymentService();
    $payService->sweepAllAgentBalances();

    $newTotalBalance = Agent::all()->sum(function ($agent) {
        return $agent->getSatsBalanceAttribute();
    });

    // Then the balances of the agents should be zero
    $this->assertEquals(0, $newTotalBalance);

});
