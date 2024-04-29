<?php

use App\Services\NostrService;

test('can connect to nostr service via grpc', function () {
    $res  = (new NostrService())
    ->poolAddress('oa.forkforge.net:5000')
    ->query('Who is satoshi')
    ->execute();

    // Assert res is a string at least 30 characters long
    // expect($res)->toBeString()->toBeGreaterThan(30);
    expect($res)->toBeString();
    expect(strlen($res))->toBeGreaterThan(30);
})->skipOnLinux();

test('testing for RAG request', function () {
    $documents = ['https://bitcoin.org/bitcoin.pdf'];

    $res  = (new NostrService())
    ->poolAddress('oa.forkforge.net:5000')
    ->query('Who is satoshi')
    ->documents($documents)
    ->k(1)
    ->maxTokens(512)
    ->overlap(256)
    ->encryptFor('')
    ->execute();
    // Assert res is a string at least 30 characters long
    // expect($res)->toBeString()->toBeGreaterThan(30);
    expect($res)->toBeString();
    expect(strlen($res))->toBeGreaterThan(30);
})->skipOnLinux();
