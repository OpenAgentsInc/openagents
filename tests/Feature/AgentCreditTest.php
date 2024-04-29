<?php

use App\Models\User;

test('interacting with agent deducts credit', function () {

});

test('no interaction possible with agent with no credit', function () {

    // Given we have a user with no credit
    $user = User::factory()->create(['credit' => 0]);

});
