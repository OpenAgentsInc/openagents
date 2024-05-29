<?php

use App\Enums\Currency;
use App\Models\Agent;
use App\Models\User;

test('when user chats with agent, sats are deducted from user balance', function () {
    $user = User::factory()->withBalance(100000, Currency::BTC)->create();
    $agent = Agent::factory()->create(['sats_per_message' => 3]);

    $response = $this->actingAs($user)->postJson(route('agents.messages.store', $agent), [
        'message' => 'Hello, agent!',
    ]);

    $response->assertOk();

    $user->refresh();

    expect($user->sats_balance)->toBe(97);
});
