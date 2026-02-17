<?php

use App\AI\Tools\LightningL402FetchTool;
use App\Lightning\L402\InvoicePayer;
use App\Lightning\L402\InvoicePaymentResult;
use Illuminate\Support\Facades\Http;
use Laravel\Ai\Tools\Request;

test('fetch tool resolves endpointPreset defaults for url method body and scope', function () {
    config()->set('lightning.demo_presets.preset_test', [
        'url' => 'https://sats4ai.com/api/l402/text-generation',
        'method' => 'POST',
        'headers' => ['Content-Type' => 'application/json'],
        'body' => '{"input":[{"role":"User","content":"hello"}],"model":"Best"}',
        'scope' => 'ep212.sats4ai',
    ]);

    $tool = new LightningL402FetchTool;

    $json = $tool->handle(new Request([
        'endpointPreset' => 'preset_test',
        'maxSpendSats' => 100,
        'approvalRequired' => true,
    ]));

    $result = json_decode($json, true);

    expect($result)->toBeArray();
    expect($result['status'])->toBe('approval_requested');
    expect($result['host'])->toBe('sats4ai.com');
    expect($result['method'])->toBe('POST');
    expect($result['scope'])->toBe('ep212.sats4ai');
    expect($result['taskId'])->toBeString();
});

test('fetch tool supports built-in EP212 openagents presets', function (string $preset, string $expectedPath, string $expectedScope) {
    $tool = new LightningL402FetchTool;

    $json = $tool->handle(new Request([
        'endpointPreset' => $preset,
        'maxSpendMsats' => 100000,
        'requireApproval' => true,
    ]));

    $result = json_decode($json, true);

    expect($result)->toBeArray();
    expect($result['status'])->toBe('approval_requested');
    expect($result['host'])->toBe('l402.openagents.com');
    expect($result['url'])->toEndWith($expectedPath);
    expect($result['scope'])->toBe($expectedScope);
    expect($result['method'])->toBe('GET');
    expect($result['taskId'])->toBeString();
})->with([
    ['ep212_openagents_premium', '/ep212/premium-signal', 'ep212.openagents.premium'],
    ['ep212_openagents_expensive', '/ep212/expensive-signal', 'ep212.openagents.expensive'],
]);

test('expensive EP212 preset can block pre-payment on quoted over-cap when approval is disabled', function () {
    config()->set('lightning.l402.allowlist_hosts', ['l402.openagents.com']);

    Http::fake([
        'https://l402.openagents.com/ep212/expensive-signal' => Http::response('', 402, [
            'WWW-Authenticate' => 'L402 macaroon="macaroon_oa", invoice="lnbc2500n1toobig"',
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
            throw new RuntimeException('payment should not be attempted when quote exceeds cap');
        }
    });

    $tool = new LightningL402FetchTool;

    $json = $tool->handle(new Request([
        'endpointPreset' => 'ep212_openagents_expensive',
        'maxSpendSats' => 100,
        'requireApproval' => false,
    ]));

    $result = json_decode($json, true);

    expect($result)->toBeArray();
    expect($result['status'])->toBe('blocked');
    expect($result['denyCode'])->toBe('quoted_cost_exceeds_cap');
    expect($result['host'])->toBe('l402.openagents.com');
    expect($result['paid'])->toBeFalse();
    expect($result['cacheHit'])->toBeFalse();
});
