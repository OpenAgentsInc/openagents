<?php

use App\Models\Withdrawal;

it('has an amount', function () {
    $withdrawal = Withdrawal::factory()->create(['amount' => 1000]);
    $this->assertEquals(1000, $withdrawal->amount);
});

it('has a status', function () {
    $withdrawal = Withdrawal::factory()->create(['status' => 'pending']);
    $this->assertEquals('pending', $withdrawal->status);
});

it('has a lightning_address', function () {
    $withdrawal = Withdrawal::factory()->create(['lightning_address' => 'blah@blah.com']);
    expect($withdrawal->lightning_address)->toBe('blah@blah.com');
});
