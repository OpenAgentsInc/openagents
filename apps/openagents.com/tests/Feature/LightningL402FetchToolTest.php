<?php

use App\Lightning\L402\InvoicePayer;
use App\Lightning\L402\InvoicePaymentResult;
use App\Lightning\L402\L402Client;
use App\Models\L402Credential;
use Illuminate\Support\Facades\Http;

beforeEach(function () {
    config()->set('lightning.l402.allowlist_hosts', ['fake-l402.local']);
    config()->set('lightning.l402.invoice_payer', 'fake');
});

test('l402 client pays once, caches credential, then uses cache without paying again', function () {
    $macaroon = 'macaroon_abc';
    $invoice = 'lnbc420n1testinvoice';

    $preimage = hash('sha256', 'preimage:'.$invoice);
    $expectedAuth = 'L402 '.$macaroon.':'.$preimage;

    Http::fake(function (\Illuminate\Http\Client\Request $req) use ($expectedAuth, $macaroon, $invoice) {
        $auth = $req->header('Authorization')[0] ?? null;

        if ($auth === $expectedAuth) {
            return Http::response('premium payload', 200, ['Content-Type' => 'text/plain']);
        }

        return Http::response('', 402, [
            'WWW-Authenticate' => 'L402 macaroon="'.$macaroon.'", invoice="'.$invoice.'"',
        ]);
    });

    app()->singleton(InvoicePayer::class, fn () => new class implements InvoicePayer
    {
        public function name(): string
        {
            return 'fake';
        }

        public function payBolt11(string $invoice, int $timeoutMs, array $context = []): InvoicePaymentResult
        {
            return new InvoicePaymentResult(hash('sha256', 'preimage:'.$invoice), 'fake');
        }
    });

    $out1 = resolve(L402Client::class)->fetch(
        url: 'https://fake-l402.local/premium',
        method: 'POST',
        headers: ['Content-Type' => 'application/json'],
        body: '{"hello":"world"}',
        maxSpendSats: 100,
        scope: 'demo.fake',
    );

    expect($out1['status'])->toBe('completed');
    expect($out1['paid'])->toBeTrue();
    expect($out1['cacheHit'])->toBeFalse();
    expect($out1['amountMsats'])->toBe(42000);
    expect($out1['proofReference'])->toBe('preimage:'.substr($preimage, 0, 16));
    expect($out1['responseBodySha256'])->toBe(hash('sha256', 'premium payload'));

    expect(L402Credential::query()->count())->toBe(1);

    // Rebind payer to ensure a cache hit never attempts a payment.
    app()->singleton(InvoicePayer::class, fn () => new class implements InvoicePayer
    {
        public function name(): string
        {
            return 'should_not_pay';
        }

        public function payBolt11(string $invoice, int $timeoutMs, array $context = []): InvoicePaymentResult
        {
            throw new RuntimeException('payment should not be attempted on cache hit');
        }
    });

    $out2 = resolve(L402Client::class)->fetch(
        url: 'https://fake-l402.local/premium',
        method: 'POST',
        headers: ['Content-Type' => 'application/json'],
        body: '{"hello":"world"}',
        maxSpendSats: 100,
        scope: 'demo.fake',
    );

    expect($out2['status'])->toBe('cached');
    expect($out2['paid'])->toBeFalse();
    expect($out2['cacheHit'])->toBeTrue();
    expect($out2['responseBodySha256'])->toBe(hash('sha256', 'premium payload'));
});

test('over-cap blocks pre-payment', function () {
    $macaroon = 'macaroon_abc';
    $invoice = 'lnbc2000n1toobig'; // 200 sats

    Http::fake([
        'https://fake-l402.local/premium' => Http::response('', 402, [
            'WWW-Authenticate' => 'L402 macaroon="'.$macaroon.'", invoice="'.$invoice.'"',
        ]),
    ]);

    // If the client tries to pay, the test should fail.
    app()->singleton(InvoicePayer::class, fn () => new class implements InvoicePayer
    {
        public function name(): string
        {
            return 'should_not_pay';
        }

        public function payBolt11(string $invoice, int $timeoutMs, array $context = []): InvoicePaymentResult
        {
            throw new RuntimeException('payment should not be attempted when quoted cost exceeds cap');
        }
    });

    $out = resolve(L402Client::class)->fetch(
        url: 'https://fake-l402.local/premium',
        method: 'POST',
        headers: ['Content-Type' => 'application/json'],
        body: '{"hello":"world"}',
        maxSpendSats: 100,
        scope: 'demo.fake',
    );

    expect($out['status'])->toBe('blocked');
    expect($out['paid'])->toBeFalse();
    expect($out['denyCode'])->toBe('quoted_cost_exceeds_cap');

    expect(L402Credential::query()->count())->toBe(0);
});

test('domain allowlist blocks before making any http request', function () {
    config()->set('lightning.l402.enforce_host_allowlist', true);
    config()->set('lightning.l402.allowlist_hosts', ['some-other-host.local']);

    $called = false;

    Http::fake(function () use (&$called) {
        $called = true;

        return Http::response('should not be called', 500);
    });

    $out = resolve(L402Client::class)->fetch(
        url: 'https://fake-l402.local/premium',
        method: 'GET',
        headers: [],
        body: null,
        maxSpendSats: 100,
        scope: 'demo.fake',
    );

    expect($called)->toBeFalse();
    expect($out['status'])->toBe('blocked');
    expect($out['denyCode'])->toBe('domain_not_allowed');
});

test('domain allowlist is optional when enforcement is disabled', function () {
    config()->set('lightning.l402.enforce_host_allowlist', false);
    config()->set('lightning.l402.allowlist_hosts', ['some-other-host.local']);

    $called = false;

    Http::fake(function () use (&$called) {
        $called = true;

        return Http::response('free payload', 200, ['Content-Type' => 'text/plain']);
    });

    $out = resolve(L402Client::class)->fetch(
        url: 'https://fake-l402.local/premium',
        method: 'GET',
        headers: [],
        body: null,
        maxSpendSats: 100,
        scope: 'demo.fake',
    );

    expect($called)->toBeTrue();
    expect($out['status'])->toBe('completed');
    expect($out['paid'])->toBeFalse();
    expect($out['responseStatusCode'])->toBe(200);
});
