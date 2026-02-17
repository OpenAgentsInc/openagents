<?php

use App\AI\Tools\LightningL402ApproveTool;
use App\AI\Tools\LightningL402FetchTool;
use App\Lightning\L402\InvoicePayer;
use App\Lightning\L402\InvoicePaymentResult;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Http;
use Laravel\Ai\Tools\Request;

test('fetch tool queues approval and approve tool executes the queued payment', function () {
    config()->set('lightning.l402.allowlist_hosts', ['fake-l402.local']);
    config()->set('lightning.l402.invoice_payer', 'fake');

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
    config()->set('lightning.l402.allowlist_hosts', ['fake-l402.local']);

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
