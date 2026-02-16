<?php

use App\Lightning\L402\WwwAuthenticateParser;

test('www-authenticate parser extracts macaroon and invoice from an L402 challenge', function () {
    $p = new WwwAuthenticateParser;

    $c = $p->parseL402Challenge('L402 macaroon="mac", invoice="inv"');

    expect($c)->not->toBeNull();
    expect($c->macaroon)->toBe('mac');
    expect($c->invoice)->toBe('inv');
});

test('www-authenticate parser can find L402 inside multi-scheme headers', function () {
    $p = new WwwAuthenticateParser;

    $c = $p->parseL402Challenge('Bearer realm="x", L402 macaroon="mac", invoice="inv"');

    expect($c)->not->toBeNull();
    expect($c->macaroon)->toBe('mac');
    expect($c->invoice)->toBe('inv');
});

test('www-authenticate parser returns null for empty/missing/invalid challenges', function () {
    $p = new WwwAuthenticateParser;

    expect($p->parseL402Challenge(null))->toBeNull();
    expect($p->parseL402Challenge(''))->toBeNull();
    expect($p->parseL402Challenge('Bearer realm="x"'))->toBeNull();
    expect($p->parseL402Challenge('L402 macaroon="", invoice="inv"'))->toBeNull();
    expect($p->parseL402Challenge('L402 macaroon="mac"'))->toBeNull();
});
