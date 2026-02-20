<?php

use App\Support\KhalaImport\KhalaChatImportService;
use Illuminate\Support\Facades\DB;

function makeKhalaExportZip(array $tables): string
{
    $zipPath = tempnam(sys_get_temp_dir(), 'khala-export-');
    if ($zipPath === false) {
        throw new RuntimeException('Failed to create temp file for khala export fixture.');
    }

    $zip = new ZipArchive;
    $opened = $zip->open($zipPath, ZipArchive::OVERWRITE);
    if ($opened !== true) {
        throw new RuntimeException('Failed to open temp ZIP for khala export fixture.');
    }

    foreach ($tables as $table => $rows) {
        $lines = [];
        foreach ($rows as $row) {
            $lines[] = json_encode($row, JSON_UNESCAPED_SLASHES);
        }

        $zip->addFromString($table.'/documents.jsonl', implode("\n", $lines)."\n");
    }

    $zip->close();

    return $zipPath;
}

function buildKhalaFixtureZip(?string $email = 'legacy@example.com'): string
{
    $threadId = '11111111-1111-1111-1111-111111111111';
    $runId = '22222222-2222-2222-2222-222222222222';

    return makeKhalaExportZip([
        'users' => [
            [
                '_id' => 'users:1',
                'userId' => 'userlegacy1',
                'email' => $email,
                'createdAtMs' => 1_735_700_000_000,
                'defaultThreadId' => $threadId,
            ],
        ],
        'threads' => [
            [
                '_id' => 'threads:1',
                'threadId' => $threadId,
                'ownerId' => 'userlegacy1',
                'createdAtMs' => 1_735_700_001_000,
                'updatedAtMs' => 1_735_700_002_000,
            ],
            [
                '_id' => 'threads:anon',
                'threadId' => 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
                'createdAtMs' => 1_735_700_001_000,
                'updatedAtMs' => 1_735_700_002_000,
            ],
        ],
        'blueprints' => [
            [
                '_id' => 'blueprints:1',
                'threadId' => $threadId,
                'updatedAtMs' => 1_735_700_030_000,
                'blueprint' => [
                    'bootstrapState' => [
                        'status' => 'complete',
                        'stage' => null,
                        'templateVersion' => 5.0,
                    ],
                    'docs' => [
                        'user' => [
                            'name' => 'Chris',
                            'addressAs' => 'Chris',
                        ],
                        'identity' => [
                            'name' => 'Autopilot',
                            'vibe' => 'direct and practical',
                        ],
                        'character' => [
                            'coreTruths' => ['Prefer verification over guessing'],
                            'boundaries' => ['No irreversible changes without explicit approval'],
                            'continuity' => 'Keep a durable profile.',
                        ],
                        'rules' => [
                            'body' => 'You are Autopilot.',
                        ],
                    ],
                    'memory' => [],
                ],
            ],
        ],
        'runs' => [
            [
                '_id' => 'runs:1',
                'threadId' => $threadId,
                'runId' => $runId,
                'assistantMessageId' => '44444444-4444-4444-4444-444444444444',
                'status' => 'final',
                'cancelRequested' => false,
                'createdAtMs' => 1_735_700_010_000,
                'updatedAtMs' => 1_735_700_020_000,
            ],
        ],
        'messages' => [
            [
                '_id' => 'messages:1',
                'threadId' => $threadId,
                'messageId' => '33333333-3333-3333-3333-333333333333',
                'role' => 'user',
                'status' => 'final',
                'text' => 'Summarize my previous conversation.',
                'createdAtMs' => 1_735_700_005_000,
                'updatedAtMs' => 1_735_700_005_000,
            ],
            [
                '_id' => 'messages:2',
                'threadId' => $threadId,
                'messageId' => '44444444-4444-4444-4444-444444444444',
                'role' => 'assistant',
                'status' => 'final',
                'text' => 'Here is a summary of your previous conversation.',
                'runId' => $runId,
                'createdAtMs' => 1_735_700_020_000,
                'updatedAtMs' => 1_735_700_020_000,
            ],
        ],
        'receipts' => [
            [
                '_id' => 'receipts:1',
                'threadId' => $threadId,
                'runId' => $runId,
                'kind' => 'tool',
                'json' => [
                    'tool' => 'echo',
                    'ok' => true,
                ],
                'createdAtMs' => 1_735_700_020_500,
            ],
        ],
    ]);
}

function buildKhalaFixtureZipWithMissingEmail(): string
{
    $threadId = '11111111-1111-1111-1111-111111111111';
    $runId = '22222222-2222-2222-2222-222222222222';

    return makeKhalaExportZip([
        'users' => [
            [
                '_id' => 'users:1',
                'userId' => 'user_legacy_1',
                'email' => null,
                'createdAtMs' => 1_735_700_000_000,
                'defaultThreadId' => $threadId,
            ],
        ],
        'threads' => [
            [
                '_id' => 'threads:1',
                'threadId' => $threadId,
                'ownerId' => 'user_legacy_1',
                'createdAtMs' => 1_735_700_001_000,
                'updatedAtMs' => 1_735_700_002_000,
            ],
        ],
        'blueprints' => [
            [
                '_id' => 'blueprints:1',
                'threadId' => $threadId,
                'updatedAtMs' => 1_735_700_030_000,
                'blueprint' => [
                    'bootstrapState' => ['status' => 'complete', 'stage' => null, 'templateVersion' => 5.0],
                    'docs' => [
                        'user' => ['name' => 'Resolved User', 'addressAs' => 'Resolved'],
                        'identity' => ['name' => 'Autopilot', 'vibe' => 'calm and direct'],
                        'character' => ['coreTruths' => ['Prefer verification over guessing']],
                        'rules' => ['body' => 'You are Autopilot.'],
                    ],
                    'memory' => [],
                ],
            ],
        ],
        'runs' => [
            [
                '_id' => 'runs:1',
                'threadId' => $threadId,
                'runId' => $runId,
                'assistantMessageId' => '44444444-4444-4444-4444-444444444444',
                'status' => 'final',
                'cancelRequested' => false,
                'createdAtMs' => 1_735_700_010_000,
                'updatedAtMs' => 1_735_700_020_000,
            ],
        ],
        'messages' => [
            [
                '_id' => 'messages:1',
                'threadId' => $threadId,
                'messageId' => '33333333-3333-3333-3333-333333333333',
                'role' => 'user',
                'status' => 'final',
                'text' => 'Summarize my previous conversation.',
                'createdAtMs' => 1_735_700_005_000,
                'updatedAtMs' => 1_735_700_005_000,
            ],
            [
                '_id' => 'messages:2',
                'threadId' => $threadId,
                'messageId' => '44444444-4444-4444-4444-444444444444',
                'role' => 'assistant',
                'status' => 'final',
                'text' => 'Here is a summary of your previous conversation.',
                'runId' => $runId,
                'createdAtMs' => 1_735_700_020_000,
                'updatedAtMs' => 1_735_700_020_000,
            ],
        ],
        'receipts' => [
            [
                '_id' => 'receipts:1',
                'threadId' => $threadId,
                'runId' => $runId,
                'kind' => 'tool',
                'json' => ['tool' => 'echo', 'ok' => true],
                'createdAtMs' => 1_735_700_020_500,
            ],
        ],
    ]);
}

afterEach(function () {
    if (isset($this->khalaFixtureZip) && is_string($this->khalaFixtureZip) && file_exists($this->khalaFixtureZip)) {
        @unlink($this->khalaFixtureZip);
    }
});

test('khala:import-chat dry-run parses without writing', function () {
    $this->khalaFixtureZip = buildKhalaFixtureZip();

    $this->artisan('khala:import-chat', [
        'source' => $this->khalaFixtureZip,
        '--dry-run' => true,
        '--replace' => true,
    ])->assertExitCode(0);

    expect(DB::table('users')->count())->toBe(0)
        ->and(DB::table('threads')->count())->toBe(0)
        ->and(DB::table('messages')->count())->toBe(0)
        ->and(DB::table('runs')->count())->toBe(0)
        ->and(DB::table('run_events')->count())->toBe(0)
        ->and(DB::table('agent_conversations')->count())->toBe(0)
        ->and(DB::table('agent_conversation_messages')->count())->toBe(0)
        ->and(DB::table('autopilots')->count())->toBe(0)
        ->and(DB::table('autopilot_profiles')->count())->toBe(0)
        ->and(DB::table('autopilot_policies')->count())->toBe(0);
});

test('khala:import-chat imports users, autopilot profile/policy, and runtime linkage', function () {
    $this->khalaFixtureZip = buildKhalaFixtureZip();

    $this->artisan('khala:import-chat', [
        'source' => $this->khalaFixtureZip,
        '--replace' => true,
    ])->assertExitCode(0);

    $user = DB::table('users')->where('workos_id', 'userlegacy1')->first();

    expect($user)->not()->toBeNull();

    $autopilot = DB::table('autopilots')->where('owner_user_id', $user->id)->first();
    expect($autopilot)->not()->toBeNull();

    $profile = DB::table('autopilot_profiles')->where('autopilot_id', $autopilot->id)->first();
    $policy = DB::table('autopilot_policies')->where('autopilot_id', $autopilot->id)->first();

    expect($profile)->not()->toBeNull();
    expect($policy)->not()->toBeNull();
    expect($profile->owner_display_name)->toBe('Chris');

    $thread = DB::table('threads')->where('id', '11111111-1111-1111-1111-111111111111')->first();
    expect($thread)->not()->toBeNull();
    expect((int) $thread->user_id)->toBe((int) $user->id);
    expect((string) $thread->autopilot_id)->toBe((string) $autopilot->id);

    $run = DB::table('runs')->where('id', '22222222-2222-2222-2222-222222222222')->first();
    expect($run)->not()->toBeNull();
    expect($run->status)->toBe('completed');
    expect((string) $run->autopilot_id)->toBe((string) $autopilot->id);
    expect((int) $run->autopilot_config_version)->toBe((int) $autopilot->config_version);

    expect(DB::table('messages')->where('thread_id', '11111111-1111-1111-1111-111111111111')->count())->toBe(2)
        ->and(DB::table('messages')->where('thread_id', '11111111-1111-1111-1111-111111111111')->where('autopilot_id', $autopilot->id)->count())->toBe(2)
        ->and(DB::table('agent_conversation_messages')->where('conversation_id', '11111111-1111-1111-1111-111111111111')->count())->toBe(2);

    $event = DB::table('run_events')->where('run_id', '22222222-2222-2222-2222-222222222222')->first();
    expect($event)->not()->toBeNull();
    expect($event->type)->toBe('legacy_receipt_tool');
    expect((string) $event->autopilot_id)->toBe((string) $autopilot->id);
    expect((string) $event->actor_type)->toBe('autopilot');
    expect((string) $event->actor_autopilot_id)->toBe((string) $autopilot->id);

    // Anon thread is intentionally skipped.
    expect(DB::table('threads')->where('id', 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa')->count())->toBe(0);
});

test('khala:import-chat resolves missing emails via WorkOS lookup when enabled', function () {
    $this->khalaFixtureZip = buildKhalaFixtureZipWithMissingEmail();

    $workosUser = (object) [
        'email' => 'resolved-user@openagents.com',
        'first_name' => 'Resolved',
        'last_name' => 'User',
        'profile_picture_url' => 'https://example.com/avatar.png',
    ];

    $workos = \Mockery::mock('overload:WorkOS\\UserManagement');
    $workos->shouldReceive('getUser')
        ->once()
        ->with('user_legacy_1')
        ->andReturn($workosUser);

    /** @var KhalaChatImportService $service */
    $service = app(KhalaChatImportService::class);

    $stats = $service->import(
        sourcePath: $this->khalaFixtureZip,
        replace: true,
        dryRun: false,
        logger: null,
        resolveWorkosUsers: true,
        importBlueprints: true,
    );

    expect($stats['users_resolved_via_workos'])->toBe(1)
        ->and($stats['users_unresolved_placeholder'])->toBe(0);

    $user = DB::table('users')->where('workos_id', 'user_legacy_1')->first(['email', 'name', 'avatar']);
    expect($user)->not()->toBeNull();
    expect($user->email)->toBe('resolved-user@openagents.com');
    expect($user->name)->toBe('Resolved User');
    expect((string) $user->avatar)->toContain('example.com/avatar.png');
});

test('khala:import-chat falls back to placeholder on email conflict and remains idempotent', function () {
    $this->khalaFixtureZip = buildKhalaFixtureZipWithMissingEmail();

    // Existing user already owns the WorkOS-resolved email.
    DB::table('users')->insert([
        'name' => 'Existing User',
        'email' => 'resolved-user@openagents.com',
        'email_verified_at' => now(),
        'workos_id' => 'user_existing',
        'avatar' => 'https://example.com/existing.png',
        'remember_token' => null,
        'created_at' => now(),
        'updated_at' => now(),
        'handle' => 'existing-user',
    ]);

    $workosUser = (object) [
        'email' => 'resolved-user@openagents.com',
        'first_name' => 'Resolved',
        'last_name' => 'User',
    ];

    $workos = \Mockery::mock('overload:WorkOS\\UserManagement');
    $workos->shouldReceive('getUser')
        ->atLeast()->once()
        ->with('user_legacy_1')
        ->andReturn($workosUser);

    /** @var KhalaChatImportService $service */
    $service = app(KhalaChatImportService::class);

    $first = $service->import(
        sourcePath: $this->khalaFixtureZip,
        replace: false,
        dryRun: false,
        logger: null,
        resolveWorkosUsers: true,
        importBlueprints: true,
    );

    $second = $service->import(
        sourcePath: $this->khalaFixtureZip,
        replace: false,
        dryRun: false,
        logger: null,
        resolveWorkosUsers: true,
        importBlueprints: true,
    );

    $legacy = DB::table('users')->where('workos_id', 'user_legacy_1')->first(['email']);

    expect($legacy)->not()->toBeNull();
    expect((string) $legacy->email)->toEndWith('@openagents.local');

    expect($first['users_email_conflicts'])->toBeGreaterThan(0)
        ->and($second['users_email_conflicts'])->toBeGreaterThanOrEqual(0)
        ->and(DB::table('users')->where('workos_id', 'user_legacy_1')->count())->toBe(1)
        ->and(DB::table('autopilots')->count())->toBe(1)
        ->and(DB::table('autopilot_profiles')->count())->toBe(1)
        ->and(DB::table('autopilot_policies')->count())->toBe(1)
        ->and(DB::table('threads')->where('id', '11111111-1111-1111-1111-111111111111')->count())->toBe(1)
        ->and(DB::table('runs')->where('id', '22222222-2222-2222-2222-222222222222')->count())->toBe(1)
        ->and(DB::table('messages')->where('thread_id', '11111111-1111-1111-1111-111111111111')->count())->toBe(2)
        ->and(DB::table('run_events')->where('run_id', '22222222-2222-2222-2222-222222222222')->count())->toBe(1);
});
