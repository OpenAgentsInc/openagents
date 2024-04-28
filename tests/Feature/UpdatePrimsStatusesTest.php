<?php

use App\Models\PrismSinglePayment;
use Mockery;
use Mockery\MockInterface;

it('updates Prism payment status', function () {
    $this->instance(PrismSinglePayment::class, function (MockInterface $mock) {
        $payment = new PrismSinglePayment(['payment_id' => 1, 'status' => 'sending']);
        $payment->payment_id = 1;
        $payment->status = 'sending';
        $testPayments = collect([$payment]);

        $mock->shouldReceive('where')
            ->once()
            ->andReturnSelf()
            ->shouldReceive('get')
            ->andReturn($testPayments);
    });

    // Call the command and assert it runs successfully
    $this->artisan('prism:update')->assertExitCode(0);

    Mockery::close();
});
