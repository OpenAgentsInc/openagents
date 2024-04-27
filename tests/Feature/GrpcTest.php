<?php

use App\Services\NostrService;

test('can connect to nostr service via grpc', function () {
    $service = new NostrService();
    $res = $service->requestContext('oa.forkforge.net:5000', 'Testing');

    // Assert res is a string at least 30 characters long
    // expect($res)->toBeString()->toBeGreaterThan(30);
    expect($res)->toBeString();
    expect(strlen($res))->toBeGreaterThan(30);
});

test('testing for RAG request', function () {
    $documents = ['https://bitcoin.org/bitcoin.pdf'];
    $service = new NostrService();
    $res = $service->requestContext('oa.forkforge.net:5000', 'Who is satoshi', $documents, 1, 256, 128, '');
    // Assert res is a string at least 30 characters long
    // expect($res)->toBeString()->toBeGreaterThan(30);
    expect($res)->toBeString();
    expect(strlen($res))->toBeGreaterThan(30);
});
