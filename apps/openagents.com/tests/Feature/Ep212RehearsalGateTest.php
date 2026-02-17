<?php

use App\AI\Tools\LightningL402ApproveTool;
use App\AI\Tools\LightningL402FetchTool;
use App\Lightning\L402\L402Client;
use Illuminate\Support\Facades\Http;
use Laravel\Ai\Tools\Request;

beforeEach(function () {
    config()->set('lightning.l402.allowlist_hosts', ['fake-l402.local']);
    config()->set('lightning.l402.invoice_payer', 'fake');
});

test('ep212 deterministic matrix: paid success', function () {
    $macaroon = 'macaroon_ep212_success';
    $invoice = 'lnbc420n1ep212success';

    $preimage = hash('sha256', 'preimage:'.$invoice);
    $expectedAuth = 'L402 '.$macaroon.':'.$preimage;

    $requestCount = 0;

    Http::fake(function (\Illuminate\Http\Client\Request $request) use (&$requestCount, $expectedAuth, $macaroon, $invoice) {
        $requestCount++;

        $auth = $request->header('Authorization')[0] ?? null;

        if ($auth === $expectedAuth) {
            return Http::response('ep212 premium payload', 200, ['Content-Type' => 'text/plain']);
        }

        return Http::response('', 402, [
            'WWW-Authenticate' => 'L402 macaroon="'.$macaroon.'", invoice="'.$invoice.'"',
        ]);
    });

    $result = resolve(L402Client::class)->fetch(
        url: 'https://fake-l402.local/ep212/premium',
        method: 'POST',
        headers: ['Content-Type' => 'application/json'],
        body: '{"prompt":"Give me one short Bitcoin fact"}',
        maxSpendSats: 100,
        scope: 'ep212.matrix.success',
    );

    expect($result['status'])->toBe('completed');
    expect($result['paid'])->toBeTrue();
    expect($result['cacheHit'])->toBeFalse();
    expect($result['host'])->toBe('fake-l402.local');
    expect($result['amountMsats'])->toBe(42000);
    expect($result['responseStatusCode'])->toBe(200);
    expect($result['responseBodySha256'])->toBe(hash('sha256', 'ep212 premium payload'));
    expect($requestCount)->toBe(2);
});

test('ep212 deterministic matrix: cached repeat', function () {
    $macaroon = 'macaroon_ep212_cache';
    $invoice = 'lnbc420n1ep212cache';

    $preimage = hash('sha256', 'preimage:'.$invoice);
    $expectedAuth = 'L402 '.$macaroon.':'.$preimage;

    $requestCount = 0;

    Http::fake(function (\Illuminate\Http\Client\Request $request) use (&$requestCount, $expectedAuth, $macaroon, $invoice) {
        $requestCount++;

        $auth = $request->header('Authorization')[0] ?? null;
        if ($auth === $expectedAuth) {
            return Http::response('ep212 cached payload', 200, ['Content-Type' => 'text/plain']);
        }

        return Http::response('', 402, [
            'WWW-Authenticate' => 'L402 macaroon="'.$macaroon.'", invoice="'.$invoice.'"',
        ]);
    });

    $client = resolve(L402Client::class);

    $first = $client->fetch(
        url: 'https://fake-l402.local/ep212/premium',
        method: 'POST',
        headers: ['Content-Type' => 'application/json'],
        body: '{"prompt":"Fetch premium signal"}',
        maxSpendSats: 100,
        scope: 'ep212.matrix.cache',
    );

    $second = $client->fetch(
        url: 'https://fake-l402.local/ep212/premium',
        method: 'POST',
        headers: ['Content-Type' => 'application/json'],
        body: '{"prompt":"Fetch premium signal"}',
        maxSpendSats: 100,
        scope: 'ep212.matrix.cache',
    );

    expect($first['status'])->toBe('completed');
    expect($first['paid'])->toBeTrue();
    expect($first['cacheHit'])->toBeFalse();

    expect($second['status'])->toBe('cached');
    expect($second['paid'])->toBeFalse();
    expect($second['cacheHit'])->toBeTrue();
    expect($second['cacheStatus'])->toBe('hit');

    expect($requestCount)->toBe(3);
});

test('ep212 deterministic matrix: blocked pre-payment', function () {
    $macaroon = 'macaroon_ep212_blocked';
    $invoice = 'lnbc420n1ep212blocked';

    $requestCount = 0;

    Http::fake(function () use (&$requestCount, $macaroon, $invoice) {
        $requestCount++;

        return Http::response('', 402, [
            'WWW-Authenticate' => 'L402 macaroon="'.$macaroon.'", invoice="'.$invoice.'"',
        ]);
    });

    $result = resolve(L402Client::class)->fetch(
        url: 'https://fake-l402.local/ep212/expensive',
        method: 'GET',
        headers: [],
        body: null,
        maxSpendSats: 10,
        scope: 'ep212.matrix.blocked',
    );

    expect($result['status'])->toBe('blocked');
    expect($result['denyCode'])->toBe('quoted_cost_exceeds_cap');
    expect($result['paid'])->toBeFalse();
    expect($result['cacheHit'])->toBeFalse();
    expect($result['quotedAmountMsats'])->toBe(42000);
    expect($result['maxSpendMsats'])->toBe(10000);
    expect($requestCount)->toBe(1);
});

test('ep212 deterministic matrix: approval lifecycle', function () {
    $macaroon = 'macaroon_ep212_approval';
    $invoice = 'lnbc420n1ep212approval';

    $preimage = hash('sha256', 'preimage:'.$invoice);
    $expectedAuth = 'L402 '.$macaroon.':'.$preimage;

    Http::fake(function (\Illuminate\Http\Client\Request $request) use ($expectedAuth, $macaroon, $invoice) {
        $auth = $request->header('Authorization')[0] ?? null;

        if ($auth === $expectedAuth) {
            return Http::response('ep212 approval payload', 200, ['Content-Type' => 'text/plain']);
        }

        return Http::response('', 402, [
            'WWW-Authenticate' => 'L402 macaroon="'.$macaroon.'", invoice="'.$invoice.'"',
        ]);
    });

    $fetch = new LightningL402FetchTool;
    $approve = new LightningL402ApproveTool;

    $queued = json_decode($fetch->handle(new Request([
        'url' => 'https://fake-l402.local/ep212/approval',
        'method' => 'POST',
        'headers' => ['Content-Type' => 'application/json'],
        'body' => '{"prompt":"approval flow"}',
        'maxSpendSats' => 100,
        'scope' => 'ep212.matrix.approval',
        'approvalRequired' => true,
    ])), true);

    expect($queued['status'])->toBe('approval_requested');
    expect($queued['approvalRequired'])->toBeTrue();
    expect($queued['taskId'])->toBeString();

    $approved = json_decode($approve->handle(new Request([
        'taskId' => $queued['taskId'],
    ])), true);

    expect($approved['status'])->toBe('completed');
    expect($approved['paid'])->toBeTrue();
    expect($approved['taskId'])->toBe($queued['taskId']);

    $again = json_decode($approve->handle(new Request([
        'taskId' => $queued['taskId'],
    ])), true);

    expect($again['status'])->toBe('failed');
    expect($again['denyCode'])->toBe('task_not_found');
});
