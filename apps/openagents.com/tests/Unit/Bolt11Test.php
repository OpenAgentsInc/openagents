<?php

use App\Lightning\L402\Bolt11;

test('bolt11 amountMsats parses common multipliers', function () {
    expect(Bolt11::amountMsats('lnbc420n1test'))->toBe(42000); // 42 sats
    expect(Bolt11::amountMsats('LNBC1m1TEST'))->toBe(100000000); // 100k sats
    expect(Bolt11::amountMsats('lnbc2500u1test'))->toBe(2500 * 100000); // 250k sats
    expect(Bolt11::amountMsats('lnbc1n1test'))->toBe(100); // 0.1 sat
    expect(Bolt11::amountMsats('lnbc10p1test'))->toBe(1); // 1 msat
    expect(Bolt11::amountMsats('lnbc1000p1test'))->toBe(100); // 100 msats
});

test('bolt11 amountMsats returns null when invoice has no amount', function () {
    expect(Bolt11::amountMsats('lnbc1amountless'))->toBeNull();
});

test('bolt11 amountMsats returns null on invalid formats and multipliers', function () {
    expect(Bolt11::amountMsats('not-an-invoice'))->toBeNull();
    expect(Bolt11::amountMsats('lnbc10x1test'))->toBeNull();
    expect(Bolt11::amountMsats('lnbc1p1test'))->toBeNull(); // pico requires 0.1msat increments
});

test('bolt11 amountMsats returns null on overflow', function () {
    $unitMsats = 100000000000;
    $digits = (string) (intdiv(PHP_INT_MAX, $unitMsats) + 1);

    expect(Bolt11::amountMsats('lnbc'.$digits.'1test'))->toBeNull();
});
