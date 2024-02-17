<?php

use App\Services\Bitcoin;

it('can fetch usd price', function () {
    $price = Bitcoin::getUsdPrice();
    expect($price)->toBeFloat();
});

it('can fetch invoice for lightning address', function () {
    $invoice = Bitcoin::requestInvoiceForLightningAddress([
        'lightning_address' => 'atlantispleb@getalby.com',
        'amount' => 1000,
        'memo' => 'OpenAgents Test Withdrawal',
    ]);
    expect($invoice)->toBeArray();
    expect($invoice)->toHaveKeys(['pr', 'routes', 'status', 'successAction', 'verify']);
    expect($invoice['status'])->toBe('OK');
})->group('integration');
