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

Artisan::command('convex:import-chat {source : Path to Convex export ZIP or directory} {--replace : Truncate target chat tables before import} {--dry-run : Parse and map without writing} {--resolve-workos-users : Resolve missing user emails via WorkOS User Management API} {--skip-blueprints : Skip blueprint-to-autopilot migration}', function () {
    $source = (string) $this->argument('source');
    $replace = (bool) $this->option('replace');
    $dryRun = (bool) $this->option('dry-run');
    $resolveWorkosUsers = (bool) $this->option('resolve-workos-users');
    $skipBlueprints = (bool) $this->option('skip-blueprints');

    /** @var ConvexChatImportService $service */
    $service = resolve(ConvexChatImportService::class);

    $this->info('Convex chat import starting...');
    $this->line('  source: '.$source);
    $this->line('  mode: '.($dryRun ? 'dry-run' : 'write'));
    $this->line('  replace: '.($replace ? 'yes' : 'no'));
    $this->line('  resolve_workos_users: '.($resolveWorkosUsers ? 'yes' : 'no'));
    $this->line('  import_blueprints: '.($skipBlueprints ? 'no' : 'yes'));

    $stats = $service->import(
        sourcePath: $source,
        replace: $replace,
        dryRun: $dryRun,
        logger: fn (string $message) => $this->line('  '.$message),
        resolveWorkosUsers: $resolveWorkosUsers,
        importBlueprints: ! $skipBlueprints,
    );

    $this->newLine();
    $this->info('Convex chat import summary:');

    foreach ($stats as $key => $value) {
        $this->line(sprintf('  %-32s %d', $key.':', $value));
    }

    if ($dryRun) {
        $this->warn('Dry-run completed: no database writes were performed.');
    } else {
        $this->info('Import completed successfully.');
    }

    return 0;
})->purpose('Import Convex users/threads/runs/messages/receipts + optional blueprints into Laravel chat/autopilot tables.');

Artisan::command('ops:test-login-link {email : Allowlisted email to log in as} {--minutes=30 : Signed URL expiry in minutes} {--name= : Optional display name override} {--base-url= : Optional base URL override}', function () {
    $email = strtolower(trim((string) $this->argument('email')));

    if (! filter_var($email, FILTER_VALIDATE_EMAIL)) {
        $this->error('Invalid email address.');

        return 1;
    }

    $minutes = (int) $this->option('minutes');
    if ($minutes < 1 || $minutes > 1440) {
        $this->error('--minutes must be between 1 and 1440.');

        return 1;
    }

    $query = ['email' => $email];
    $name = trim((string) $this->option('name'));
    if ($name !== '') {
        $query['name'] = $name;
    }

    $signedUrl = \Illuminate\Support\Facades\URL::temporarySignedRoute(
        'internal.test-login',
        now()->addMinutes($minutes),
        $query,
    );

    $baseUrl = rtrim((string) $this->option('base-url'), '/');
    if ($baseUrl !== '') {
        $parts = parse_url($signedUrl);
        $path = (string) ($parts['path'] ?? '/');
        $queryString = isset($parts['query']) ? ('?'.$parts['query']) : '';
        $signedUrl = $baseUrl.$path.$queryString;
    }

    $this->line('Signed maintenance test-login URL:');
    $this->line($signedUrl);

    if (! config('auth.local_test_login.enabled', false)) {
        $this->warn('OA_ALLOW_LOCAL_TEST_AUTH is disabled in this environment. Enable it for this URL to work.');
    }

    return 0;
})->purpose('Generate a temporary signed test-login URL for maintenance-mode verification.');

Artisan::command('runtime:tools:invoke-api {--api-base=http://127.0.0.1:8000 : Base URL for the Laravel API host} {--token= : Sanctum token used to call /api/runtime/tools/execute} {--tool-pack=coding.v1 : Runtime tool-pack name} {--mode=replay : execute or replay} {--operation=get_issue : coding operation (get_issue|get_pull_request|add_issue_comment)} {--repository=OpenAgentsInc/openagents : target repository owner/repo} {--issue-number=1 : issue number for issue/comment operations} {--pull-number=1 : pull request number for get_pull_request} {--comment-body= : body for add_issue_comment} {--run-id=run_cli_tools : optional run id context} {--thread-id=thread_cli_tools : optional thread id context} {--write-approved=0 : set 1 to approve write operation}', function () {
    $token = trim((string) $this->option('token'));
    if ($token === '') {
        $this->error('--token is required');

        return 1;
    }

    $apiBase = rtrim((string) $this->option('api-base'), '/');
    if ($apiBase === '') {
        $this->error('--api-base must be a valid URL');

        return 1;
    }

    $operation = trim((string) $this->option('operation'));
    $repository = trim((string) $this->option('repository'));
    $mode = trim((string) $this->option('mode'));
    $runId = trim((string) $this->option('run-id'));
    $threadId = trim((string) $this->option('thread-id'));
    $writeApproved = ((string) $this->option('write-approved')) === '1';

    $requestPayload = [
        'integration_id' => 'github.primary',
        'operation' => $operation,
        'repository' => $repository,
        'run_id' => $runId,
        'thread_id' => $threadId,
        'tool_call_id' => 'tool_call_cli_'.time(),
    ];

    if ($operation === 'get_pull_request') {
        $requestPayload['pull_number'] = max(1, (int) $this->option('pull-number'));
    } else {
        $requestPayload['issue_number'] = max(1, (int) $this->option('issue-number'));
    }

    if ($operation === 'add_issue_comment') {
        $commentBody = trim((string) $this->option('comment-body'));
        if ($commentBody === '') {
            $this->error('--comment-body is required for add_issue_comment');

            return 1;
        }
        $requestPayload['body'] = $commentBody;
    }

    $payload = [
        'tool_pack' => trim((string) $this->option('tool-pack')),
        'mode' => $mode === '' ? 'replay' : $mode,
        'run_id' => $runId === '' ? null : $runId,
        'thread_id' => $threadId === '' ? null : $threadId,
        'manifest' => [
            'manifest_version' => 'coding.integration.v1',
            'integration_id' => 'github.primary',
            'provider' => 'github',
            'status' => 'active',
            'tool_pack' => 'coding.v1',
            'capabilities' => ['get_issue', 'get_pull_request', 'add_issue_comment'],
            'secrets_ref' => ['provider' => 'laravel', 'key_id' => 'intsec_github_1'],
            'policy' => [
                'write_operations_mode' => 'enforce',
                'max_requests_per_minute' => 240,
                'default_repository' => $repository,
            ],
        ],
        'request' => $requestPayload,
        'policy' => [
            'authorization_id' => 'auth_cli_demo',
            'authorization_mode' => 'delegated_budget',
            'write_approved' => $writeApproved,
            'budget' => [
                'max_total_sats' => 10_000,
                'max_per_call_sats' => 2_000,
            ],
        ],
    ];

    $url = $apiBase.'/api/runtime/tools/execute';
    $response = Http::withToken($token)->acceptJson()->timeout(30)->post($url, $payload);

    $this->line('Runtime tools API status: '.$response->status());

    $json = $response->json();
    if (is_array($json)) {
        $this->line(json_encode($json, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES) ?: '{}');
    } else {
        $this->line((string) $response->body());
    }

    return $response->successful() ? 0 : 1;
})->purpose('Call /api/runtime/tools/execute with a coding payload to validate runtime tool invocation through Laravel.');

Artisan::command('ops:create-api-token {email : Email of the existing user} {name=ops-cli : Token display name} {--abilities=* : Comma-separated abilities} {--expires-days= : Optional token expiration in days}', function () {
    $email = strtolower(trim((string) $this->argument('email')));
    $name = trim((string) $this->argument('name'));

    if (! filter_var($email, FILTER_VALIDATE_EMAIL)) {
        $this->error('Invalid email address.');

        return 1;
    }

    if ($name === '') {
        $this->error('Token name cannot be empty.');

        return 1;
    }

    $user = \App\Models\User::query()->where('email', $email)->first();
    if (! $user) {
        $this->error('User not found for email: '.$email);

        return 1;
    }

    $abilitiesRaw = $this->option('abilities');
    $abilityInputs = is_array($abilitiesRaw)
        ? $abilitiesRaw
        : [$abilitiesRaw];

    $abilities = collect($abilityInputs)
        ->filter(static fn (mixed $ability): bool => is_string($ability))
        ->flatMap(static fn (string $ability): array => explode(',', $ability))
        ->map(static fn (string $ability): string => trim($ability))
        ->filter(static fn (string $ability): bool => $ability !== '')
        ->values()
        ->all();

    if ($abilities === []) {
        $abilities = ['*'];
    }
    $expiresAt = null;
    $expiresDaysOption = trim((string) $this->option('expires-days'));
    if ($expiresDaysOption !== '') {
        $days = (int) $expiresDaysOption;
        if ($days < 1 || $days > 3650) {
            $this->error('--expires-days must be between 1 and 3650 when provided.');

            return 1;
        }

        $expiresAt = now()->addDays($days);
    }

    $token = $user->createToken($name, $abilities, $expiresAt);

    $this->line('Token created successfully. Copy now; it will not be shown again:');
    $this->line($token->plainTextToken);
    $this->line('');
    $this->line('metadata:');
    $this->line('  user_id='.$user->id);
    $this->line('  email='.$user->email);
    $this->line('  name='.$name);
    $this->line('  abilities='.implode(',', $abilities));
    $this->line('  expires_at='.($expiresAt?->toISOString() ?? 'null'));

    return 0;
})->purpose('Create a Sanctum API token for an existing user.');
