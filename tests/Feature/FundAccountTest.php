<?php

use App\Billing\FakePaymentGateway;
use App\Billing\PaymentGateway;
use App\Models\User;

// before each test, set up
beforeEach(function () {
    $this->user = User::factory()->create();
    $this->actingAs($this->user);
    $this->paymentGateway = new FakePaymentGateway;
    $this->app->instance(PaymentGateway::class, $this->paymentGateway);
});

test('user can fund their account', function () {
    $this->post('/fund', [
        'payment_token' => $this->paymentGateway->getValidTestToken(),
        'amount' => 1000,
    ]);

    $this->assertEquals(1000, $this->paymentGateway->totalCharges());
})->skip();
