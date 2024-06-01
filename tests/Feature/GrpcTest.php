<?php

use App\Services\PoolService;

test('can connect to nostr service via grpc', function () {
    $res = (new PoolService())
        ->poolAddress(env('POOL_ADDRESS'))
        ->query('Who is satoshi')
        ->execute();

    // Assert res is a string at least 30 characters long
    // expect($res)->toBeString()->toBeGreaterThan(30);
    expect($res)->toBeString();
    expect(strlen($res))->toBeGreaterThan(30);
})->skip(empty(env('POOL_ADDRESS')));

test('testing for RAG request', function () {
    $documents = ['https://bitcoin.org/bitcoin.pdf'];

    $res = (new PoolService())
        ->poolAddress(env('POOL_ADDRESS'))
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
})->skip(empty(env('POOL_ADDRESS')));

test('testing for RAG request warm up for the node server', function () {
    $documents = ['https://bitcoin.org/bitcoin.pdf'];

    $res = (new PoolService())
        ->poolAddress(env('POOL_ADDRESS'))
        ->query('Who is satoshi')
        ->documents($documents)
        ->k(1)
        ->maxTokens(512)
        ->overlap(256)
        ->encryptFor('')
        ->warmUp(true)
        ->cacheDurationhint('-1')
        ->execute();
    // Assert res is a string at least 30 characters long
    // expect($res)->toBeString()->toBeGreaterThan(30);
    expect($res)->toBeString();
    expect(strlen($res))->toBeGreaterThan(30);
})->skip(empty(env('POOL_ADDRESS')));
