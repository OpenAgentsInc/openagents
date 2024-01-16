<?php

test('bitcoin page loads', function () {
    $this->get('/bitcoin')->assertOk()->assertViewIs('bitcoin');
});

test('we see bitcoin price', function () {
    $this->get('/bitcoin')->assertSee('BTCUSD $43000');
});
