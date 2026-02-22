<?php

use App\Lightning\L402\InvoicePayer;
use App\Lightning\L402\InvoicePayers\FakeInvoicePayer;
use App\Lightning\L402\L402Client;
use App\Support\KhalaImport\KhalaChatImportService;
use Illuminate\Foundation\Inspiring;
use Illuminate\Support\Facades\Artisan;
use Illuminate\Support\Facades\File;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Route;

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

Artisan::command('khala:import-chat {source : Path to Khala export ZIP or directory} {--replace : Truncate target chat tables before import} {--dry-run : Parse and map without writing} {--resolve-workos-users : Resolve missing user emails via WorkOS User Management API} {--skip-blueprints : Skip blueprint-to-autopilot migration}', function () {
    $source = (string) $this->argument('source');
    $replace = (bool) $this->option('replace');
    $dryRun = (bool) $this->option('dry-run');
    $resolveWorkosUsers = (bool) $this->option('resolve-workos-users');
    $skipBlueprints = (bool) $this->option('skip-blueprints');

    /** @var KhalaChatImportService $service */
    $service = resolve(KhalaChatImportService::class);

    $this->info('Khala chat import starting...');
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
    $this->info('Khala chat import summary:');

    foreach ($stats as $key => $value) {
        $this->line(sprintf('  %-32s %d', $key.':', $value));
    }

    if ($dryRun) {
        $this->warn('Dry-run completed: no database writes were performed.');
    } else {
        $this->info('Import completed successfully.');
    }

    return 0;
})->purpose('Import Khala users/threads/runs/messages/receipts + optional blueprints into Laravel chat/autopilot tables.');

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

Artisan::command('ops:export-parity-manifests {--output=docs/parity-manifests/baseline : Output directory for generated manifests (absolute or relative to project root)}', function () {
    $outputOption = trim((string) $this->option('output'));
    if ($outputOption === '') {
        $this->error('--output cannot be empty.');

        return 1;
    }

    $outputDir = str_starts_with($outputOption, DIRECTORY_SEPARATOR)
        ? $outputOption
        : base_path($outputOption);
    $outputDir = rtrim($outputDir, DIRECTORY_SEPARATOR);
    File::ensureDirectoryExists($outputDir);

    $generatedAt = now()->toISOString();
    $routeRows = collect(Route::getRoutes()->getRoutes())
        ->flatMap(function ($route): array {
            $uri = '/'.ltrim($route->uri(), '/');
            $routeName = $route->getName();
            $actionName = $route->getActionName();
            $middleware = array_values($route->gatherMiddleware());

            return collect($route->methods())
                ->reject(fn (string $method): bool => in_array($method, ['HEAD', 'OPTIONS'], true))
                ->map(fn (string $method): array => [
                    'method' => $method,
                    'uri' => $uri,
                    'name' => $routeName,
                    'action' => $actionName,
                    'middleware' => $middleware,
                ])
                ->values()
                ->all();
        })
        ->sortBy(fn (array $route): string => sprintf('%s %s', $route['method'], $route['uri']))
        ->values();

    $apiRoutes = $routeRows
        ->filter(fn (array $route): bool => str_starts_with($route['uri'], '/api/'))
        ->values();

    $webRoutes = $routeRows
        ->filter(fn (array $route): bool => ! str_starts_with($route['uri'], '/api/'))
        ->values();

    $pagesRoot = resource_path('js/pages');
    $pageEntries = collect(File::exists($pagesRoot) ? File::allFiles($pagesRoot) : [])
        ->filter(function (\SplFileInfo $file): bool {
            $extension = strtolower($file->getExtension());

            return in_array($extension, ['tsx', 'jsx', 'ts', 'js', 'vue'], true);
        })
        ->map(function (\SplFileInfo $file): string {
            $relative = str_replace(base_path().DIRECTORY_SEPARATOR, '', $file->getPathname());

            return str_replace(DIRECTORY_SEPARATOR, '/', $relative);
        })
        ->sort()
        ->values();

    $consoleSource = File::exists(base_path('routes/console.php'))
        ? File::get(base_path('routes/console.php'))
        : '';
    preg_match_all("/Artisan::command\\('([^']+)'/", $consoleSource, $signatureMatches);
    $customCommandNames = collect($signatureMatches[1] ?? [])
        ->map(fn (string $signature): string => (string) preg_split('/\\s+/', trim($signature))[0])
        ->filter(fn (string $name): bool => $name !== '')
        ->unique()
        ->sort()
        ->values();

    $allCommands = collect(Artisan::all());
    $customCommands = $customCommandNames
        ->map(function (string $name) use ($allCommands): array {
            $command = $allCommands->get($name);

            return [
                'name' => $name,
                'description' => $command?->getDescription() ?? '',
                'hidden' => $command?->isHidden() ?? false,
                'class' => $command ? get_class($command) : null,
            ];
        })
        ->values();

    $writeJson = function (string $fileName, array $payload) use ($outputDir): string {
        $path = $outputDir.DIRECTORY_SEPARATOR.$fileName;
        $encoded = json_encode($payload, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES);
        if (! is_string($encoded)) {
            throw new RuntimeException('Failed to encode '.$fileName.' as JSON.');
        }
        File::put($path, $encoded.PHP_EOL);

        return $path;
    };

    $apiPath = $writeJson('api-routes.json', [
        'generated_at' => $generatedAt,
        'count' => $apiRoutes->count(),
        'routes' => $apiRoutes->all(),
    ]);
    $webPath = $writeJson('web-routes.json', [
        'generated_at' => $generatedAt,
        'count' => $webRoutes->count(),
        'routes' => $webRoutes->all(),
    ]);
    $pagesPath = $writeJson('page-entries.json', [
        'generated_at' => $generatedAt,
        'count' => $pageEntries->count(),
        'entries' => $pageEntries->all(),
    ]);
    $commandsPath = $writeJson('artisan-commands.json', [
        'generated_at' => $generatedAt,
        'count' => $customCommands->count(),
        'commands' => $customCommands->all(),
    ]);
    $indexPath = $writeJson('manifest-index.json', [
        'generated_at' => $generatedAt,
        'manifests' => [
            ['name' => 'api-routes', 'path' => basename($apiPath), 'count' => $apiRoutes->count()],
            ['name' => 'web-routes', 'path' => basename($webPath), 'count' => $webRoutes->count()],
            ['name' => 'page-entries', 'path' => basename($pagesPath), 'count' => $pageEntries->count()],
            ['name' => 'artisan-commands', 'path' => basename($commandsPath), 'count' => $customCommands->count()],
        ],
    ]);

    $this->info('Parity manifests exported.');
    $this->line('  output: '.$outputDir);
    $this->line('  api routes: '.$apiRoutes->count());
    $this->line('  web routes: '.$webRoutes->count());
    $this->line('  page entries: '.$pageEntries->count());
    $this->line('  artisan commands: '.$customCommands->count());
    $this->line('  index: '.$indexPath);

    return 0;
})->purpose('Export baseline JSON manifests for API routes, web routes, page entries, and custom Artisan commands.');

Artisan::command('ops:capture-parity-contract-fixtures {--output=docs/parity-fixtures/baseline : Output directory for golden fixtures (absolute or relative to project root)} {--openapi-temp=storage/app/openapi-parity-capture.json : Temporary OpenAPI output path (absolute or relative to project root)}', function () {
    $outputOption = trim((string) $this->option('output'));
    if ($outputOption === '') {
        $this->error('--output cannot be empty.');

        return 1;
    }
    $outputDir = str_starts_with($outputOption, DIRECTORY_SEPARATOR)
        ? $outputOption
        : base_path($outputOption);
    $outputDir = rtrim($outputDir, DIRECTORY_SEPARATOR);
    File::ensureDirectoryExists($outputDir);

    $openapiOption = trim((string) $this->option('openapi-temp'));
    if ($openapiOption === '') {
        $this->error('--openapi-temp cannot be empty.');

        return 1;
    }
    $openapiPath = str_starts_with($openapiOption, DIRECTORY_SEPARATOR)
        ? $openapiOption
        : base_path($openapiOption);
    File::ensureDirectoryExists(dirname($openapiPath));

    $status = Artisan::call('openapi:generate', ['--output' => $openapiPath]);
    if ($status !== 0) {
        $this->error('openapi:generate failed while capturing parity fixtures.');

        return $status;
    }

    if (! File::exists($openapiPath)) {
        $this->error('OpenAPI output was not created at '.$openapiPath);

        return 1;
    }

    $openapiRaw = File::get($openapiPath);
    $openapi = json_decode($openapiRaw, true);
    if (! is_array($openapi)) {
        $this->error('Failed to decode OpenAPI output as JSON.');

        return 1;
    }

    $extractJsonExample = function (array $content): mixed {
        if (! isset($content['application/json']) || ! is_array($content['application/json'])) {
            return null;
        }

        $json = $content['application/json'];
        if (array_key_exists('example', $json)) {
            return $json['example'];
        }

        if (isset($json['examples']) && is_array($json['examples'])) {
            $firstExample = reset($json['examples']);
            if (is_array($firstExample) && array_key_exists('value', $firstExample)) {
                return $firstExample['value'];
            }
        }

        if (isset($json['schema']) && is_array($json['schema']) && array_key_exists('example', $json['schema'])) {
            return $json['schema']['example'];
        }

        return null;
    };

    $methods = ['get', 'post', 'patch', 'delete', 'put'];
    $httpFixtures = [];
    $paths = $openapi['paths'] ?? [];
    if (! is_array($paths)) {
        $paths = [];
    }

    ksort($paths);
    foreach ($paths as $path => $operations) {
        if (! is_array($operations)) {
            continue;
        }

        foreach ($methods as $method) {
            $operation = $operations[$method] ?? null;
            if (! is_array($operation)) {
                continue;
            }

            $requestBody = $operation['requestBody'] ?? [];
            if (! is_array($requestBody)) {
                $requestBody = [];
            }

            $requestContent = $requestBody['content'] ?? [];
            if (! is_array($requestContent)) {
                $requestContent = [];
            }

            $responses = $operation['responses'] ?? [];
            if (! is_array($responses)) {
                $responses = [];
            }

            ksort($responses);
            $responseFixtures = [];
            foreach ($responses as $statusCode => $responseSpec) {
                if (! is_array($responseSpec)) {
                    continue;
                }
                $content = $responseSpec['content'] ?? [];
                if (! is_array($content)) {
                    $content = [];
                }

                $responseFixtures[] = [
                    'status' => (string) $statusCode,
                    'description' => $responseSpec['description'] ?? '',
                    'json_example' => $extractJsonExample($content),
                ];
            }

            $fixtureId = strtoupper($method).' '.$path;
            $httpFixtures[] = [
                'id' => $fixtureId,
                'method' => strtoupper($method),
                'path' => $path,
                'summary' => $operation['summary'] ?? '',
                'tags' => array_values(array_filter($operation['tags'] ?? [], fn (mixed $tag): bool => is_string($tag))),
                'request_json_example' => $extractJsonExample($requestContent),
                'responses' => $responseFixtures,
            ];
        }
    }

    $repoRoot = realpath(base_path('../..'));
    if (! is_string($repoRoot) || $repoRoot === '') {
        $this->error('Failed to resolve repository root for protocol fixtures.');

        return 1;
    }

    $fixturePaths = [
        'khala_frame' => $repoRoot.'/docs/protocol/fixtures/khala-frame-v1.json',
        'codex_worker_events' => $repoRoot.'/docs/protocol/fixtures/codex-worker-events-v1.json',
        'control_auth_session' => $repoRoot.'/docs/protocol/fixtures/control-auth-session-v1.json',
    ];

    $loadedFixtures = [];
    foreach ($fixturePaths as $key => $path) {
        if (! File::exists($path)) {
            $this->error('Missing protocol fixture: '.$path);

            return 1;
        }
        $decoded = json_decode((string) File::get($path), true);
        if (! is_array($decoded)) {
            $this->error('Failed to decode protocol fixture: '.$path);

            return 1;
        }
        $loadedFixtures[$key] = $decoded;
    }

    $generatedAt = now()->toISOString();
    $httpGolden = [
        'schema' => 'openagents.webparity.http_golden.v1',
        'generated_at' => $generatedAt,
        'source' => [
            'openapi' => str_replace(base_path().DIRECTORY_SEPARATOR, '', $openapiPath),
            'capture_command' => 'ops:capture-parity-contract-fixtures',
        ],
        'fixture_count' => count($httpFixtures),
        'fixtures' => $httpFixtures,
        'auth_contract_samples' => $loadedFixtures['control_auth_session'],
    ];

    $khalaFrames = $loadedFixtures['khala_frame']['fixtures'] ?? [];
    if (! is_array($khalaFrames)) {
        $khalaFrames = [];
    }

    $workerEvents = $loadedFixtures['codex_worker_events']['notification_events'] ?? [];
    if (! is_array($workerEvents)) {
        $workerEvents = [];
    }

    $replayEvents = array_values(array_filter(
        $workerEvents,
        fn (mixed $event): bool => is_array($event) && (($event['replay']['replayed'] ?? false) === true)
    ));

    $khalaGolden = [
        'schema' => 'openagents.webparity.khala_ws_golden.v1',
        'generated_at' => $generatedAt,
        'source' => [
            'khala_frame_fixture' => 'docs/protocol/fixtures/khala-frame-v1.json',
            'codex_worker_events_fixture' => 'docs/protocol/fixtures/codex-worker-events-v1.json',
        ],
        'frame_count' => count($khalaFrames),
        'frames' => $khalaFrames,
        'worker_summary' => $loadedFixtures['codex_worker_events']['worker_summary'] ?? null,
        'worker_snapshot' => $loadedFixtures['codex_worker_events']['worker_snapshot'] ?? null,
        'replay_event_count' => count($replayEvents),
        'replay_events' => $replayEvents,
        'live_event_count' => count($workerEvents) - count($replayEvents),
        'all_events' => $workerEvents,
    ];

    $writeJson = function (string $fileName, array $payload) use ($outputDir): string {
        $path = $outputDir.DIRECTORY_SEPARATOR.$fileName;
        $encoded = json_encode($payload, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES);
        if (! is_string($encoded)) {
            throw new RuntimeException('Failed to encode '.$fileName.' as JSON.');
        }
        File::put($path, $encoded.PHP_EOL);

        return $path;
    };

    $httpPath = $writeJson('http-json-golden.json', $httpGolden);
    $khalaPath = $writeJson('khala-ws-golden.json', $khalaGolden);
    $indexPath = $writeJson('capture-index.json', [
        'generated_at' => $generatedAt,
        'artifacts' => [
            [
                'name' => 'http-json-golden',
                'path' => basename($httpPath),
                'fixture_count' => count($httpFixtures),
            ],
            [
                'name' => 'khala-ws-golden',
                'path' => basename($khalaPath),
                'frame_count' => count($khalaFrames),
                'event_count' => count($workerEvents),
            ],
        ],
    ]);

    $this->info('Parity contract fixtures captured.');
    $this->line('  output: '.$outputDir);
    $this->line('  http fixtures: '.count($httpFixtures));
    $this->line('  khala frames: '.count($khalaFrames));
    $this->line('  khala events: '.count($workerEvents));
    $this->line('  index: '.$indexPath);

    return 0;
})->purpose('Capture golden HTTP JSON fixtures and Khala WS transcript fixtures for Rust parity conformance.');
