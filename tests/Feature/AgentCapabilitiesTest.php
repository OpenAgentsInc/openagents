<?php

use App\Models\Agent;

test('can check an agent capabilities', function () {

    // The default agent has no capabilities
    $agent = Agent::factory()->create();
    expect($agent->hasCapability('create-threads'))->toBeFalse();

});
