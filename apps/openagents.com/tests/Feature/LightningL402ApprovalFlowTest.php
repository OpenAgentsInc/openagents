<?php

use App\AI\Runtime\AutopilotExecutionContext;
use App\AI\Tools\LightningL402ApproveTool;
use App\AI\Tools\LightningL402FetchTool;
use App\Lightning\L402\InvoicePayer;
use App\Lightning\L402\InvoicePaymentResult;
use App\Models\AutopilotPolicy;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Str;
use Laravel\Ai\Tools\Request;

beforeEach(function () {
    config()->set('lightning.l402.allowlist_hosts', ['fake-l402.local']);
    config()->set('lightning.l402.invoice_payer', 'fake');
});

test('fetch tool queues approval and approve tool executes the queued payment', function () {
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

    $fetchTool = new LightningL402FetchTool;
    $approveTool = new LightningL402ApproveTool;

    $queuedJson = $fetchTool->handle(new Request([
        'url' => 'https://fake-l402.local/premium',
        'method' => 'POST',
        'headers' => ['Content-Type' => 'application/json'],
        'body' => '{"hello":"world"}',
        'maxSpendSats' => 100,
        'scope' => 'demo.fake',
        'approvalRequired' => true,
    ]));

    $queued = json_decode($queuedJson, true);

    expect($queued)->toBeArray();
    expect($queued['status'])->toBe('approval_requested');
    expect($queued['taskId'])->toBeString();
    expect($queued['approvalRequired'])->toBeTrue();
    expect($queued['requireApproval'])->toBeTrue();

    $taskId = $queued['taskId'];

    $resultJson = $approveTool->handle(new Request([
        'taskId' => $taskId,
    ]));

    $result = json_decode($resultJson, true);

    expect($result)->toBeArray();
    expect($result['status'])->toBe('completed');
    expect($result['paid'])->toBeTrue();
    expect($result['host'])->toBe('fake-l402.local');
    expect($result['taskId'])->toBe($taskId);

    $row = DB::table('l402_pending_approvals')->where('id', $taskId)->first();
    expect($row)->not->toBeNull();
    expect($row->status)->toBe('consumed');

    $againJson = $approveTool->handle(new Request([
        'taskId' => $taskId,
    ]));

    $again = json_decode($againJson, true);
    expect($again['status'])->toBe('failed');
    expect($again['denyCode'])->toBe('task_not_found');
});

test('approve tool returns task_expired when queued intent is expired', function () {
    $fetchTool = new LightningL402FetchTool;
    $approveTool = new LightningL402ApproveTool;

    $queuedJson = $fetchTool->handle(new Request([
        'url' => 'https://fake-l402.local/premium',
        'maxSpendSats' => 100,
        'scope' => 'demo.fake',
        'approvalRequired' => true,
    ]));

    $queued = json_decode($queuedJson, true);
    $taskId = $queued['taskId'];

    DB::table('l402_pending_approvals')
        ->where('id', $taskId)
        ->update([
            'expires_at' => now()->subMinute(),
            'updated_at' => now(),
        ]);

    $resultJson = $approveTool->handle(new Request([
        'taskId' => $taskId,
    ]));

    $result = json_decode($resultJson, true);

    expect($result['status'])->toBe('failed');
    expect($result['denyCode'])->toBe('task_expired');
});

test('fetch tool honors canonical l402 fields and keeps aliases during migration', function () {
    $fetchTool = new LightningL402FetchTool;

    $queuedJson = $fetchTool->handle(new Request([
        'url' => 'https://fake-l402.local/premium',
        'method' => 'POST',
        'headers' => ['Content-Type' => 'application/json'],
        'body' => '{"hello":"world"}',
        'maxSpendMsats' => 55_000,
        'maxSpendSats' => 100,
        'scope' => 'demo.fake',
        'requireApproval' => true,
        'approvalRequired' => false,
    ]));

    $queued = json_decode($queuedJson, true);

    expect($queued['status'])->toBe('approval_requested');
    expect($queued['maxSpendMsats'])->toBe(55_000);
    expect($queued['maxSpendSats'])->toBe(55);
    expect($queued['requireApproval'])->toBeTrue();
    expect($queued['approvalRequired'])->toBeTrue();
});

test('autopilot policy enforces host cap and approval before payment', function () {
    $fetchTool = new LightningL402FetchTool;
    $context = resolve(AutopilotExecutionContext::class);
    $autopilotId = (string) Str::uuid7();

    AutopilotPolicy::query()->create([
        'autopilot_id' => $autopilotId,
        'l402_require_approval' => true,
        'l402_max_spend_msats_per_call' => 50_000,
        'l402_allowed_hosts' => ['fake-l402.local'],
    ]);

    $context->set(123, $autopilotId);

    $blockedJson = $fetchTool->handle(new Request([
        'url' => 'https://fake-l402.local/premium',
        'maxSpendMsats' => 100_000,
        'scope' => 'demo.fake',
        'requireApproval' => false,
    ]));

    $blocked = json_decode($blockedJson, true);

    expect($blocked['status'])->toBe('blocked');
    expect($blocked['denyCode'])->toBe('max_spend_exceeds_policy_cap');
    expect($blocked['paid'])->toBeFalse();

    $queuedJson = $fetchTool->handle(new Request([
        'url' => 'https://fake-l402.local/premium',
        'maxSpendMsats' => 50_000,
        'scope' => 'demo.fake',
        'requireApproval' => false,
    ]));

    $queued = json_decode($queuedJson, true);

    expect($queued['status'])->toBe('approval_requested');
    expect($queued['requireApproval'])->toBeTrue();
    expect($queued['approvalRequired'])->toBeTrue();

    $deniedHostJson = $fetchTool->handle(new Request([
        'url' => 'https://denied.example/premium',
        'maxSpendMsats' => 50_000,
        'scope' => 'demo.fake',
        'requireApproval' => false,
    ]));

    $deniedHost = json_decode($deniedHostJson, true);

    expect($deniedHost['status'])->toBe('blocked');
    expect($deniedHost['denyCode'])->toBe('domain_not_allowed');

    $context->clear();
});

test('approve tool rechecks autopilot policy before payment execution', function () {
    Http::fake();

    app()->singleton(InvoicePayer::class, fn () => new class implements InvoicePayer
    {
        public function name(): string
        {
            return 'should_not_pay';
        }

        public function payBolt11(string $invoice, int $timeoutMs, array $context = []): InvoicePaymentResult
        {
            throw new \RuntimeException('payment should not be attempted when policy blocks approval task');
        }
    });

    $fetchTool = new LightningL402FetchTool;
    $approveTool = new LightningL402ApproveTool;
    $context = resolve(AutopilotExecutionContext::class);
    $autopilotId = (string) Str::uuid7();

    AutopilotPolicy::query()->create([
        'autopilot_id' => $autopilotId,
        'l402_require_approval' => true,
        'l402_max_spend_msats_per_call' => 100_000,
        'l402_allowed_hosts' => ['fake-l402.local'],
    ]);

    $context->set(123, $autopilotId);

    $queuedJson = $fetchTool->handle(new Request([
        'url' => 'https://fake-l402.local/premium',
        'maxSpendMsats' => 100_000,
        'scope' => 'demo.fake',
        'requireApproval' => true,
    ]));

    $queued = json_decode($queuedJson, true);
    expect($queued['status'])->toBe('approval_requested');

    AutopilotPolicy::query()
        ->where('autopilot_id', $autopilotId)
        ->update([
            'l402_max_spend_msats_per_call' => 50_000,
            'updated_at' => now(),
        ]);

    $context->clear();

    $resultJson = $approveTool->handle(new Request([
        'taskId' => $queued['taskId'],
    ]));

    $result = json_decode($resultJson, true);

    expect($result['status'])->toBe('blocked');
    expect($result['denyCode'])->toBe('max_spend_exceeds_policy_cap');
    expect($result['paid'])->toBeFalse();

    Http::assertNothingSent();
});
