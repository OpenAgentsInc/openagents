<?php

use App\Services\Bitcoin;

test('bitcoin page loads', function () {
    $this->get('/bitcoin')->assertOk()->assertViewIs('bitcoin');
});

test('we see bitcoin price', function () {
    $this->get('/bitcoin')->assertSee('BTCUSD $43000');
});

test('we see actual bitcoin price', function () {
    $price = Bitcoin::getUsdPrice();
    $this->get('/bitcoin')->assertSee("BTCUSD \${$price}");
});
