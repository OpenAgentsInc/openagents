<?php

use App\Lightning\L402\InvoicePayers\LndRestInvoicePayer;
use Illuminate\Support\Facades\Http;

beforeEach(function () {
    config()->set('lightning.lnd_rest.tls_cert_base64', null);
    config()->set('lightning.lnd_rest.tls_verify', true);
});

test('lnd rest payer throws when missing config', function () {
    config()->set('lightning.lnd_rest.base_url', '');
    config()->set('lightning.lnd_rest.macaroon_hex', '');

    $payer = new LndRestInvoicePayer;

    expect(fn () => $payer->payBolt11('lnbc1invoice', 12000))
        ->toThrow(RuntimeException::class, 'LND REST payer not configured');
});

test('lnd rest payer normalizes hex preimages', function () {
    config()->set('lightning.lnd_rest.base_url', 'https://lnd.local');
    config()->set('lightning.lnd_rest.macaroon_hex', 'deadbeef');
    config()->set('lightning.lnd_rest.tls_verify', false);

    Http::fake([
        'https://lnd.local/v1/channels/transactions' => Http::response([
            'payment_preimage' => strtoupper(str_repeat('a', 64)),
            'payment_hash' => 'hash_1',
            'payment_error' => '',
        ], 200),
    ]);

    $payer = new LndRestInvoicePayer;
    $res = $payer->payBolt11('lnbc1invoice', 12000);

    expect($res->preimage)->toBe(str_repeat('a', 64));
    expect($res->paymentId)->toBe('hash_1');

    Http::assertSent(function (\Illuminate\Http\Client\Request $req) {
        return $req->hasHeader('Grpc-Metadata-macaroon')
            && ($req->header('Grpc-Metadata-macaroon')[0] ?? null) === 'deadbeef'
            && $req->url() === 'https://lnd.local/v1/channels/transactions';
    });
});

test('lnd rest payer supports base64 preimages', function () {
    config()->set('lightning.lnd_rest.base_url', 'https://lnd.local');
    config()->set('lightning.lnd_rest.macaroon_hex', 'deadbeef');
    config()->set('lightning.lnd_rest.tls_verify', false);

    $bytes = random_bytes(32);
    $b64 = base64_encode($bytes);

    Http::fake([
        'https://lnd.local/v1/channels/transactions' => Http::response([
            'payment_preimage' => $b64,
            'payment_hash' => 'hash_2',
            'payment_error' => '',
        ], 200),
    ]);

    $payer = new LndRestInvoicePayer;
    $res = $payer->payBolt11('lnbc1invoice', 12000);

    expect($res->preimage)->toBe(bin2hex($bytes));
    expect($res->paymentId)->toBe('hash_2');
});

test('lnd rest payer throws on payment_error', function () {
    config()->set('lightning.lnd_rest.base_url', 'https://lnd.local');
    config()->set('lightning.lnd_rest.macaroon_hex', 'deadbeef');
    config()->set('lightning.lnd_rest.tls_verify', false);

    Http::fake([
        'https://lnd.local/v1/channels/transactions' => Http::response([
            'payment_preimage' => str_repeat('a', 64),
            'payment_hash' => 'hash_err',
            'payment_error' => 'insufficient_balance',
        ], 200),
    ]);

    $payer = new LndRestInvoicePayer;

    expect(fn () => $payer->payBolt11('lnbc1invoice', 12000))
        ->toThrow(RuntimeException::class, 'insufficient_balance');
});

test('lnd rest payer throws on missing payment_preimage', function () {
    config()->set('lightning.lnd_rest.base_url', 'https://lnd.local');
    config()->set('lightning.lnd_rest.macaroon_hex', 'deadbeef');
    config()->set('lightning.lnd_rest.tls_verify', false);

    Http::fake([
        'https://lnd.local/v1/channels/transactions' => Http::response([
            'payment_hash' => 'hash_missing',
            'payment_error' => '',
        ], 200),
    ]);

    $payer = new LndRestInvoicePayer;

    expect(fn () => $payer->payBolt11('lnbc1invoice', 12000))
        ->toThrow(RuntimeException::class, 'missing payment_preimage');
});

test('lnd rest payer throws when payment_preimage is neither hex nor base64', function () {
    config()->set('lightning.lnd_rest.base_url', 'https://lnd.local');
    config()->set('lightning.lnd_rest.macaroon_hex', 'deadbeef');
    config()->set('lightning.lnd_rest.tls_verify', false);

    Http::fake([
        'https://lnd.local/v1/channels/transactions' => Http::response([
            'payment_preimage' => '***not-base64***',
            'payment_hash' => 'hash_bad',
            'payment_error' => '',
        ], 200),
    ]);

    $payer = new LndRestInvoicePayer;

    expect(fn () => $payer->payBolt11('lnbc1invoice', 12000))
        ->toThrow(RuntimeException::class, 'neither hex nor base64');
});

test('invalid base64 tls cert config is rejected', function () {
    config()->set('lightning.lnd_rest.base_url', 'https://lnd.local');
    config()->set('lightning.lnd_rest.macaroon_hex', 'deadbeef');
    config()->set('lightning.lnd_rest.tls_cert_base64', 'not base64');

    Http::fake();

    $payer = new LndRestInvoicePayer;

    expect(fn () => $payer->payBolt11('lnbc1invoice', 12000))
        ->toThrow(RuntimeException::class, 'Invalid base64');
});
