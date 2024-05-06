<?php

use App\Models\Agent;

test('can check an agent capabilities', function () {
    // The default agent has no capabilities
    $agent = Agent::factory()->create();
    expect($agent->hasCapability('codebase_search'))->toBeFalse();

    // Let's create an agent with capabilities
    $agent = Agent::factory()->create([
        'capabilities' => json_encode(['codebase_search']),
    ]);
    expect($agent->hasCapability('codebase_search'))->toBeTrue();
});
