<?php

use App\Models\User;

it('has credits', function () {
    $user = User::factory()->create(['credits' => 0]);
    $this->assertEquals(0, $user->credits);
});
