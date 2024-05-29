<?php

use App\Models\Payment;

test('explorer route loads', function () {
    $this->get('/explorer')
        ->assertStatus(200)
        ->assertSee('Explorer');
});

test('explorer page shows the most recent payments', function () {
    // Run PaymentSeeder
    $this->artisan('db:seed', ['--class' => 'PaymentSeeder']);

    // Fetch the 10 most recent payments
    $recentPayments = Payment::latest()->take(10)->get();

    $response = $this->get('/explorer');

    $response->assertStatus(200)
        ->assertSee('Recent payments');

    // Assert we see the amounts of 10 recent payments
    foreach ($recentPayments as $payment) {
        $response->assertSee($payment->amount);
    }
});
