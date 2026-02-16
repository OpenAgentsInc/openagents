<?php

use Illuminate\Support\Facades\DB;

function makeConvexExportZip(array $tables): string
{
    $zipPath = tempnam(sys_get_temp_dir(), 'convex-export-');
    if ($zipPath === false) {
        throw new RuntimeException('Failed to create temp file for convex export fixture.');
    }

    $zip = new ZipArchive;
    $opened = $zip->open($zipPath, ZipArchive::OVERWRITE);
    if ($opened !== true) {
        throw new RuntimeException('Failed to open temp ZIP for convex export fixture.');
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

function buildConvexFixtureZip(): string
{
    $threadId = '11111111-1111-1111-1111-111111111111';
    $runId = '22222222-2222-2222-2222-222222222222';

    return makeConvexExportZip([
        'users' => [
            [
                '_id' => 'users:1',
                'userId' => 'user_legacy_1',
                'email' => 'legacy@example.com',
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
            [
                '_id' => 'threads:anon',
                'threadId' => 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
                'createdAtMs' => 1_735_700_001_000,
                'updatedAtMs' => 1_735_700_002_000,
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

afterEach(function () {
    if (isset($this->convexFixtureZip) && is_string($this->convexFixtureZip) && file_exists($this->convexFixtureZip)) {
        @unlink($this->convexFixtureZip);
    }
});

test('convex:import-chat dry-run parses without writing', function () {
    $this->convexFixtureZip = buildConvexFixtureZip();

    $this->artisan('convex:import-chat', [
        'source' => $this->convexFixtureZip,
        '--dry-run' => true,
        '--replace' => true,
    ])->assertExitCode(0);

    expect(DB::table('users')->count())->toBe(0)
        ->and(DB::table('threads')->count())->toBe(0)
        ->and(DB::table('messages')->count())->toBe(0)
        ->and(DB::table('runs')->count())->toBe(0)
        ->and(DB::table('run_events')->count())->toBe(0)
        ->and(DB::table('agent_conversations')->count())->toBe(0)
        ->and(DB::table('agent_conversation_messages')->count())->toBe(0);
});

test('convex:import-chat imports users, conversations, messages, runs, and receipt events', function () {
    $this->convexFixtureZip = buildConvexFixtureZip();

    $this->artisan('convex:import-chat', [
        'source' => $this->convexFixtureZip,
        '--replace' => true,
    ])->assertExitCode(0);

    $user = DB::table('users')->where('workos_id', 'user_legacy_1')->first();

    expect($user)->not()->toBeNull();

    $thread = DB::table('threads')->where('id', '11111111-1111-1111-1111-111111111111')->first();
    expect($thread)->not()->toBeNull();
    expect((int) $thread->user_id)->toBe((int) $user->id);

    $conversation = DB::table('agent_conversations')->where('id', '11111111-1111-1111-1111-111111111111')->first();
    expect($conversation)->not()->toBeNull();
    expect((int) $conversation->user_id)->toBe((int) $user->id);

    $run = DB::table('runs')->where('id', '22222222-2222-2222-2222-222222222222')->first();
    expect($run)->not()->toBeNull();
    expect($run->status)->toBe('completed');

    expect(DB::table('messages')->where('thread_id', '11111111-1111-1111-1111-111111111111')->count())->toBe(2)
        ->and(DB::table('agent_conversation_messages')->where('conversation_id', '11111111-1111-1111-1111-111111111111')->count())->toBe(2);

    $event = DB::table('run_events')->where('run_id', '22222222-2222-2222-2222-222222222222')->first();
    expect($event)->not()->toBeNull();
    expect($event->type)->toBe('legacy_receipt_tool');

    // Anon thread is intentionally skipped.
    expect(DB::table('threads')->where('id', 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa')->count())->toBe(0);
});

test('convex:import-chat is idempotent for repeated imports', function () {
    $this->convexFixtureZip = buildConvexFixtureZip();

    $this->artisan('convex:import-chat', [
        'source' => $this->convexFixtureZip,
        '--replace' => true,
    ])->assertExitCode(0);

    $this->artisan('convex:import-chat', [
        'source' => $this->convexFixtureZip,
    ])->assertExitCode(0);

    expect(DB::table('users')->where('workos_id', 'user_legacy_1')->count())->toBe(1)
        ->and(DB::table('threads')->where('id', '11111111-1111-1111-1111-111111111111')->count())->toBe(1)
        ->and(DB::table('agent_conversations')->where('id', '11111111-1111-1111-1111-111111111111')->count())->toBe(1)
        ->and(DB::table('runs')->where('id', '22222222-2222-2222-2222-222222222222')->count())->toBe(1)
        ->and(DB::table('messages')->where('thread_id', '11111111-1111-1111-1111-111111111111')->count())->toBe(2)
        ->and(DB::table('agent_conversation_messages')->where('conversation_id', '11111111-1111-1111-1111-111111111111')->count())->toBe(2)
        ->and(DB::table('run_events')->where('run_id', '22222222-2222-2222-2222-222222222222')->count())->toBe(1);
});
