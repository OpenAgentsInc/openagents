<?php

use App\Models\User;

it('has credit', function () {
    $user = User::factory()->create(['credit' => 0]);
    $this->assertEquals(0, $user->credit);
});
