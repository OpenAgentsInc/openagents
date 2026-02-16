<?php

use App\Lightning\L402\InvoicePayer;
use App\Lightning\L402\InvoicePayers\FakeInvoicePayer;
use App\Lightning\L402\L402Client;
use App\Support\ConvexImport\ConvexChatImportService;
use Illuminate\Foundation\Inspiring;
use Illuminate\Support\Facades\Artisan;
use Illuminate\Support\Facades\Http;

Artisan::command('inspire', function () {
    $this->comment(Inspiring::quote());
})->purpose('Display an inspiring quote');

Artisan::command('demo:l402 {--preset=fake : Endpoint preset name (fake|sats4ai)} {--max-spend-sats=100 : Hard cap in sats}', function () {
    $presetName = (string) $this->option('preset');
    $maxSpendSats = (int) $this->option('max-spend-sats');

    $preset = config('lightning.demo_presets.'.$presetName);
    if (! is_array($preset)) {
        $this->error('Unknown preset: '.$presetName);

        return 1;
    }

    // Deterministic in-process fake seller for local demos + CI.
    if ($presetName === 'fake') {
        config()->set('lightning.l402.allowlist_hosts', ['fake-l402.local']);
        config()->set('lightning.l402.invoice_payer', 'fake');

        $macaroon = 'macaroon_demo';
        $invoice = 'lnbc420n1demo';
        $preimage = hash('sha256', 'preimage:'.$invoice);
        $expectedAuth = 'L402 '.$macaroon.':'.$preimage;

        Http::fake(function (\Illuminate\Http\Client\Request $req) use ($expectedAuth, $macaroon, $invoice) {
            $auth = $req->header('Authorization')[0] ?? null;

            if ($auth === $expectedAuth) {
                return Http::response('demo premium payload', 200, ['Content-Type' => 'text/plain']);
            }

            return Http::response('', 402, [
                'WWW-Authenticate' => 'L402 macaroon="'.$macaroon.'", invoice="'.$invoice.'"',
            ]);
        });

        app()->singleton(InvoicePayer::class, fn () => new FakeInvoicePayer);
    }

    $out = resolve(L402Client::class)->fetch(
        url: (string) ($preset['url'] ?? ''),
        method: (string) ($preset['method'] ?? 'GET'),
        headers: is_array($preset['headers'] ?? null) ? $preset['headers'] : [],
        body: isset($preset['body']) && is_string($preset['body']) ? $preset['body'] : null,
        maxSpendSats: $maxSpendSats,
        scope: isset($preset['scope']) && is_string($preset['scope']) ? $preset['scope'] : 'default',
    );

    $host = $out['host'] ?? 'unknown';
    $status = $out['status'] ?? 'unknown';
    $paid = ($out['paid'] ?? false) ? 'paid' : 'unpaid';

    $msats = $out['amountMsats'] ?? $out['quotedAmountMsats'] ?? null;
    $sats = is_int($msats) ? (int) round($msats / 1000) : null;

    $this->info('L402 demo result');
    $this->line('  preset: '.$presetName);
    $this->line('  host: '.$host);
    $this->line('  status: '.(string) $status);
    $this->line('  paid: '.$paid);
    if ($sats !== null) {
        $this->line('  sats: '.$sats);
    }
    if (isset($out['proofReference']) && is_string($out['proofReference'])) {
        $this->line('  proof: '.$out['proofReference']);
    }
    if (isset($out['responseBodySha256']) && is_string($out['responseBodySha256'])) {
        $this->line('  response_sha256: '.$out['responseBodySha256']);
    }

    return ($out['status'] ?? '') === 'failed' ? 1 : 0;
})->purpose('Run a deterministic L402 buying demo (no browser required).');

Artisan::command('convex:import-chat {source : Path to Convex export ZIP or directory} {--replace : Truncate target chat tables before import} {--dry-run : Parse and map without writing}', function () {
    $source = (string) $this->argument('source');
    $replace = (bool) $this->option('replace');
    $dryRun = (bool) $this->option('dry-run');

    /** @var ConvexChatImportService $service */
    $service = resolve(ConvexChatImportService::class);

    $this->info('Convex chat import starting...');
    $this->line('  source: '.$source);
    $this->line('  mode: '.($dryRun ? 'dry-run' : 'write'));
    $this->line('  replace: '.($replace ? 'yes' : 'no'));

    $stats = $service->import(
        sourcePath: $source,
        replace: $replace,
        dryRun: $dryRun,
        logger: fn (string $message) => $this->line('  '.$message),
    );

    $this->newLine();
    $this->info('Convex chat import summary:');

    foreach ($stats as $key => $value) {
        $this->line(sprintf('  %-22s %d', $key.':', $value));
    }

    if ($dryRun) {
        $this->warn('Dry-run completed: no database writes were performed.');
    } else {
        $this->info('Import completed successfully.');
    }

    return 0;
})->purpose('Import users/threads/messages/runs/receipts from a Convex snapshot export into Laravel chat tables.');
