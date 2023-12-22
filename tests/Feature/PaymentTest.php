<?php

use App\Models\User;

test('system can pay a user', function () {
    $user = User::factory()->create();
    $this->assertEquals(0, $user->balance);
    $user->pay(1000);
    $this->assertEquals(1000, $user->balance);
});
