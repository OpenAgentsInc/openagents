<?php

use Illuminate\Http\Client\Request;
use Illuminate\Support\Facades\Http;

test('runtime:tools:invoke-api calls Laravel runtime tools api endpoint', function () {
    Http::fake([
        'https://openagents.test/api/runtime/tools/execute' => Http::response([
            'data' => [
                'decision' => 'allowed',
                'reason_code' => 'policy_allowed.default',
            ],
        ], 200),
    ]);

    $this->artisan('runtime:tools:invoke-api', [
        '--api-base' => 'https://openagents.test',
        '--token' => 'pat_test_123',
        '--mode' => 'replay',
        '--operation' => 'get_issue',
        '--repository' => 'OpenAgentsInc/openagents',
        '--issue-number' => 1747,
        '--run-id' => 'run_cli_1',
        '--thread-id' => 'thread_cli_1',
    ])
        ->expectsOutputToContain('Runtime tools API status: 200')
        ->expectsOutputToContain('"decision": "allowed"')
        ->assertExitCode(0);

    Http::assertSent(function (Request $request): bool {
        $authHeader = (string) ($request->header('Authorization')[0] ?? '');

        return $request->url() === 'https://openagents.test/api/runtime/tools/execute'
            && $request->method() === 'POST'
            && $authHeader === 'Bearer pat_test_123'
            && $request['tool_pack'] === 'coding.v1'
            && $request['mode'] === 'replay'
            && $request['request']['operation'] === 'get_issue'
            && $request['request']['issue_number'] === 1747;
    });
});

test('runtime:tools:invoke-api requires a token option', function () {
    $this->artisan('runtime:tools:invoke-api', [
        '--api-base' => 'https://openagents.test',
    ])
        ->expectsOutputToContain('--token is required')
        ->assertExitCode(1);
});

