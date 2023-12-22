<?php

use App\Models\User;

test('when referred user pays, referrer gets paid', function () {
    $referrer = User::factory()->create(['balance' => 0]);
    $user = User::factory()->create(['referrer_id' => $referrer->id]);

    $this->actingAs($user)
        ->post('payment', ['amount' => 1000]);

    $this->assertEquals(10, $referrer->balance);
});
