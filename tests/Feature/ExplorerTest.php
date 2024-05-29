<?php

test('explorer route loads', function () {
    $this->get('/explorer')
        ->assertStatus(200)
        ->assertSee('Explorer');
});

test('explorer page shows the most recent payments', function () {
    // Run PaymentSeeder
    $this->artisan('db:seed', ['--class' => 'PaymentSeeder']);

    $this->get('/explorer')
        ->assertStatus(200)
        ->assertSee('Recent payments');
});
