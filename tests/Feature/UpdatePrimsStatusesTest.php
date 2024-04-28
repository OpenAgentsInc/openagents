<?php

use App\Models\PrismSinglePayment;
use Mockery;

it('updates Prism payment statuses', function () {
    $mockPayment = Mockery::mock('overload:'.PrismSinglePayment::class);

    $payment = new PrismSinglePayment(['payment_id' => 1, 'status' => 'sending']);
    $payment->payment_id = 1;
    $payment->status = 'sending';

    // Set up a fake response for the where call
    $mockPayments = collect([$payment]);
    $mockPayment->shouldReceive('where')
        ->once()
        ->andReturnSelf()
        ->shouldReceive('get')
        ->andReturn($mockPayments);

    // Call the command and assert it runs successfully
    $this->artisan('prism:update')->assertExitCode(0);

    Mockery::close();
});
