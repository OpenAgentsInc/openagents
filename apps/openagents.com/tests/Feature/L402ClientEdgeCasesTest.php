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

test('missing L402 challenge returns a typed failure without attempting payment', function () {
    Http::fake([
        'https://fake-l402.local/premium' => Http::response('', 402, [
            'WWW-Authenticate' => 'Basic realm="nope"',
        ]),
    ]);

    app()->singleton(InvoicePayer::class, fn () => new class implements InvoicePayer
    {
        public function name(): string
        {
            return 'should_not_pay';
        }

        public function payBolt11(string $invoice, int $timeoutMs, array $context = []): InvoicePaymentResult
        {
            throw new RuntimeException('payment should not be attempted without a parseable L402 challenge');
        }
    });

    $out = resolve(L402Client::class)->fetch(
        url: 'https://fake-l402.local/premium',
        method: 'GET',
        headers: [],
        body: null,
        maxSpendSats: 100,
        scope: 'demo.fake',
    );

    expect($out['status'])->toBe('failed');
    expect($out['denyCode'])->toBe('missing_l402_challenge');
    expect($out['paid'])->toBeFalse();
});

test('amountless invoices are blocked pre-payment with quoted_amount_missing', function () {
    $macaroon = 'macaroon_abc';
    $invoice = 'lnbc1amountless';

    Http::fake([
        'https://fake-l402.local/premium' => Http::response('', 402, [
            'WWW-Authenticate' => 'L402 macaroon="'.$macaroon.'", invoice="'.$invoice.'"',
        ]),
    ]);

    app()->singleton(InvoicePayer::class, fn () => new class implements InvoicePayer
    {
        public function name(): string
        {
            return 'should_not_pay';
        }

        public function payBolt11(string $invoice, int $timeoutMs, array $context = []): InvoicePaymentResult
        {
            throw new RuntimeException('payment should not be attempted when quoted amount cannot be parsed');
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
    expect($out['denyCode'])->toBe('quoted_amount_missing');
    expect($out['paid'])->toBeFalse();
});

test('response capture enforces truncation and preview limits', function () {
    config()->set('lightning.l402.response_max_bytes', 10);
    config()->set('lightning.l402.response_preview_bytes', 4);

    $macaroon = 'macaroon_abc';
    $invoice = 'lnbc420n1testinvoice';

    $preimage = hash('sha256', 'preimage:'.$invoice);
    $expectedAuth = 'L402 '.$macaroon.':'.$preimage;

    $body = '0123456789abcdef';
    $expectedCaptured = substr($body, 0, 10);

    Http::fake(function (\Illuminate\Http\Client\Request $req) use ($expectedAuth, $macaroon, $invoice, $body) {
        $auth = $req->header('Authorization')[0] ?? null;

        if ($auth === $expectedAuth) {
            return Http::response($body, 200, ['Content-Type' => 'text/plain']);
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

    $out = resolve(L402Client::class)->fetch(
        url: 'https://fake-l402.local/premium',
        method: 'GET',
        headers: [],
        body: null,
        maxSpendSats: 100,
        scope: 'demo.fake',
    );

    expect($out['status'])->toBe('completed');
    expect($out['responseTruncated'])->toBeTrue();
    expect($out['responseBytes'])->toBe(10);
    expect($out['responseBodyTextPreview'])->toBe('0123');
    expect($out['responseBodySha256'])->toBe(hash('sha256', $expectedCaptured));
});

test('cache rejection invalidates the credential and retries full flow', function () {
    $macaroon1 = 'macaroon_v1';
    $invoice1 = 'lnbc420n1inv1';

    $macaroon2 = 'macaroon_v2';
    $invoice2 = 'lnbc420n1inv2';

    $preimage1 = hash('sha256', 'preimage:'.$invoice1);
    $preimage2 = hash('sha256', 'preimage:'.$invoice2);

    $expectedAuth1 = 'L402 '.$macaroon1.':'.$preimage1;
    $expectedAuth2 = 'L402 '.$macaroon2.':'.$preimage2;

    $auth1Count = 0;

    Http::fake(function (\Illuminate\Http\Client\Request $req) use (
        &$auth1Count,
        $expectedAuth1,
        $expectedAuth2,
        $macaroon1,
        $invoice1,
        $macaroon2,
        $invoice2,
    ) {
        $auth = $req->header('Authorization')[0] ?? null;

        if ($auth === $expectedAuth1) {
            $auth1Count++;

            // 1st time: success (creates the cache)
            // 2nd time: simulate cache rejection
            return $auth1Count === 1
                ? Http::response('premium payload v1', 200)
                : Http::response('forbidden', 403);
        }

        if ($auth === $expectedAuth2) {
            return Http::response('premium payload v2', 200);
        }

        // No auth: return the appropriate challenge.
        return Http::response('', 402, [
            'WWW-Authenticate' => $auth1Count === 0
                ? 'L402 macaroon="'.$macaroon1.'", invoice="'.$invoice1.'"'
                : 'L402 macaroon="'.$macaroon2.'", invoice="'.$invoice2.'"',
        ]);
    });

    $payCount = 0;

    $payFn = function (string $invoice, int $timeoutMs) use (&$payCount): InvoicePaymentResult {
        $payCount++;

        return new InvoicePaymentResult(hash('sha256', 'preimage:'.$invoice), 'fake');
    };

    app()->singleton(InvoicePayer::class, fn () => new class($payFn) implements InvoicePayer
    {
        public function __construct(private \Closure $payFn) {}

        public function name(): string
        {
            return 'fake';
        }

        public function payBolt11(string $invoice, int $timeoutMs, array $context = []): InvoicePaymentResult
        {
            $fn = $this->payFn;

            return $fn($invoice, $timeoutMs);
        }
    });

    $out1 = resolve(L402Client::class)->fetch(
        url: 'https://fake-l402.local/premium',
        method: 'GET',
        headers: [],
        body: null,
        maxSpendSats: 100,
        scope: 'demo.fake',
    );

    expect($out1['status'])->toBe('completed');
    expect($out1['paid'])->toBeTrue();
    expect(L402Credential::query()->count())->toBe(1);

    $out2 = resolve(L402Client::class)->fetch(
        url: 'https://fake-l402.local/premium',
        method: 'GET',
        headers: [],
        body: null,
        maxSpendSats: 100,
        scope: 'demo.fake',
    );

    expect($out2['status'])->toBe('completed');
    expect($out2['paid'])->toBeTrue();
    expect($out2['proofReference'])->toBe('preimage:'.substr($preimage2, 0, 16));

    expect($payCount)->toBe(2);
    expect($auth1Count)->toBe(2);

    expect(L402Credential::query()->count())->toBe(1);
    expect(L402Credential::query()->first()->macaroon)->toBe($macaroon2);
    expect(L402Credential::query()->first()->preimage)->toBe($preimage2);
});

test('non-402 responses return completed without paying', function () {
    Http::fake([
        'https://fake-l402.local/premium' => Http::response('free payload', 200, ['Content-Type' => 'text/plain']),
    ]);

    app()->singleton(InvoicePayer::class, fn () => new class implements InvoicePayer
    {
        public function name(): string
        {
            return 'should_not_pay';
        }

        public function payBolt11(string $invoice, int $timeoutMs, array $context = []): InvoicePaymentResult
        {
            throw new RuntimeException('payment should not be attempted for non-402 responses');
        }
    });

    $out = resolve(L402Client::class)->fetch(
        url: 'https://fake-l402.local/premium',
        method: 'GET',
        headers: [],
        body: null,
        maxSpendSats: 100,
        scope: 'demo.fake',
    );

    expect($out['status'])->toBe('completed');
    expect($out['paid'])->toBeFalse();
    expect($out['responseStatusCode'])->toBe(200);
    expect($out['responseBodySha256'])->toBe(hash('sha256', 'free payload'));
});
