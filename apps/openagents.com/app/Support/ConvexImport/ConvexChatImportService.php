<?php

namespace App\Support\ConvexImport;

use Illuminate\Support\Carbon;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;
use InvalidArgumentException;

final class ConvexChatImportService
{
    /**
     * @return array<string, int>
     */
    public function import(string $sourcePath, bool $replace, bool $dryRun, ?callable $logger = null): array
    {
        if (! file_exists($sourcePath)) {
            throw new InvalidArgumentException("Source path does not exist: [$sourcePath]");
        }

        $log = static function (string $message) use ($logger): void {
            if (is_callable($logger)) {
                $logger($message);
            }
        };

        $reader = new ConvexExportReader($sourcePath);

        /** @var array<int, array<string, mixed>> $sourceUsers */
        $sourceUsers = $reader->readTable('users');
        /** @var array<int, array<string, mixed>> $sourceThreads */
        $sourceThreads = $reader->readTable('threads');
        /** @var array<int, array<string, mixed>> $sourceRuns */
        $sourceRuns = $reader->readTable('runs');
        /** @var array<int, array<string, mixed>> $sourceMessages */
        $sourceMessages = $reader->readTable('messages');
        /** @var array<int, array<string, mixed>> $sourceReceipts */
        $sourceReceipts = $reader->readTable('receipts');

        $log(sprintf(
            'Loaded source rows: users=%d threads=%d runs=%d messages=%d receipts=%d',
            count($sourceUsers),
            count($sourceThreads),
            count($sourceRuns),
            count($sourceMessages),
            count($sourceReceipts),
        ));

        if ($replace && ! $dryRun) {
            $this->truncateTargetTables();
            $log('Truncated target chat tables (--replace).');
        }

        /**
         * @var array<string, array{workosId:string,email:?string,createdAtMs:?int}>
         */
        $userCandidates = [];

        foreach ($sourceUsers as $row) {
            $workosId = $this->stringOrNull($row['userId'] ?? null);
            if (! $workosId) {
                continue;
            }

            $userCandidates[$workosId] = [
                'workosId' => $workosId,
                'email' => $this->stringOrNull($row['email'] ?? null),
                'createdAtMs' => $this->intOrNull($row['createdAtMs'] ?? null),
            ];
        }

        foreach ($sourceThreads as $row) {
            $ownerId = $this->stringOrNull($row['ownerId'] ?? null);
            if (! $ownerId) {
                continue;
            }

            if (! isset($userCandidates[$ownerId])) {
                $userCandidates[$ownerId] = [
                    'workosId' => $ownerId,
                    'email' => null,
                    'createdAtMs' => $this->intOrNull($row['createdAtMs'] ?? null),
                ];
            }
        }

        $usedEmails = [];

        /** @var array<string, int> $laravelUserIdByWorkosId */
        $laravelUserIdByWorkosId = [];

        $syntheticId = 1;

        foreach ($userCandidates as $workosId => $candidate) {
            $candidateEmail = $this->preferredEmail($candidate['email'] ?? null, $workosId);
            $email = $this->uniqueEmail($candidateEmail, $workosId, $usedEmails, $dryRun);
            $name = $this->displayName($workosId, $email);
            $createdAt = $this->fromMs($candidate['createdAtMs'] ?? null);
            $avatar = $this->avatarUrl($email);

            if ($dryRun) {
                $laravelUserIdByWorkosId[$workosId] = $syntheticId++;

                continue;
            }

            $existing = DB::table('users')
                ->where('workos_id', $workosId)
                ->first(['id']);

            if ($existing) {
                $laravelUserIdByWorkosId[$workosId] = (int) $existing->id;

                continue;
            }

            $id = (int) DB::table('users')->insertGetId([
                'name' => $name,
                'email' => $email,
                'workos_id' => $workosId,
                'avatar' => $avatar,
                'email_verified_at' => null,
                'remember_token' => null,
                'created_at' => $createdAt,
                'updated_at' => $createdAt,
            ]);

            $laravelUserIdByWorkosId[$workosId] = $id;
        }

        /** @var array<string, int> $threadOwnerUserIdByThreadId */
        $threadOwnerUserIdByThreadId = [];
        /** @var array<string, array<string, mixed>> $threadRowsByThreadId */
        $threadRowsByThreadId = [];

        foreach ($sourceThreads as $row) {
            $threadId = $this->stringOrNull($row['threadId'] ?? null);
            $ownerId = $this->stringOrNull($row['ownerId'] ?? null);

            if (! $threadId || ! $ownerId) {
                continue;
            }

            $userId = $laravelUserIdByWorkosId[$ownerId] ?? null;
            if (! is_int($userId)) {
                continue;
            }

            $threadOwnerUserIdByThreadId[$threadId] = $userId;
            $threadRowsByThreadId[$threadId] = $row;
        }

        /** @var array<string, array{createdAtMs:int,text:string}> $firstUserMessageByThread */
        $firstUserMessageByThread = [];

        foreach ($sourceMessages as $row) {
            $threadId = $this->stringOrNull($row['threadId'] ?? null);
            $role = $this->stringOrNull($row['role'] ?? null);
            $text = trim((string) ($row['text'] ?? ''));
            $createdAtMs = $this->intOrNull($row['createdAtMs'] ?? null) ?? PHP_INT_MAX;

            if (! $threadId || $role !== 'user' || $text === '') {
                continue;
            }

            $existing = $firstUserMessageByThread[$threadId] ?? null;
            if (! is_array($existing) || $createdAtMs < $existing['createdAtMs']) {
                $firstUserMessageByThread[$threadId] = [
                    'createdAtMs' => $createdAtMs,
                    'text' => $text,
                ];
            }
        }

        $threadsImported = 0;

        foreach ($threadRowsByThreadId as $threadId => $row) {
            $ownerUserId = $threadOwnerUserIdByThreadId[$threadId] ?? null;
            if (! is_int($ownerUserId)) {
                continue;
            }

            $title = $this->threadTitle(
                $threadId,
                $firstUserMessageByThread[$threadId]['text'] ?? null,
            );

            $createdAt = $this->fromMs($this->intOrNull($row['createdAtMs'] ?? null));
            $updatedAt = $this->fromMs($this->intOrNull($row['updatedAtMs'] ?? null) ?? $this->intOrNull($row['createdAtMs'] ?? null));

            if (! $dryRun) {
                DB::table('threads')->updateOrInsert(
                    ['id' => $threadId],
                    [
                        'user_id' => $ownerUserId,
                        'title' => $title,
                        'created_at' => $createdAt,
                        'updated_at' => $updatedAt,
                    ],
                );

                DB::table('agent_conversations')->updateOrInsert(
                    ['id' => $threadId],
                    [
                        'user_id' => $ownerUserId,
                        'title' => $title,
                        'created_at' => $createdAt,
                        'updated_at' => $updatedAt,
                    ],
                );
            }

            $threadsImported++;
        }

        /** @var array<string, bool> $runIdsImported */
        $runIdsImported = [];
        $runsImported = 0;

        foreach ($sourceRuns as $row) {
            $threadId = $this->stringOrNull($row['threadId'] ?? null);
            $runId = $this->stringOrNull($row['runId'] ?? null);

            if (! $threadId || ! $runId || ! Str::isUuid($runId)) {
                continue;
            }

            $ownerUserId = $threadOwnerUserIdByThreadId[$threadId] ?? null;
            if (! is_int($ownerUserId)) {
                continue;
            }

            $status = $this->mapRunStatus($this->stringOrNull($row['status'] ?? null));
            $startedAt = $this->fromMs($this->intOrNull($row['createdAtMs'] ?? null));
            $completedAt = in_array($status, ['completed', 'failed', 'canceled'], true)
                ? $this->fromMs($this->intOrNull($row['updatedAtMs'] ?? null) ?? $this->intOrNull($row['createdAtMs'] ?? null))
                : null;
            $createdAt = $startedAt;
            $updatedAt = $this->fromMs($this->intOrNull($row['updatedAtMs'] ?? null) ?? $this->intOrNull($row['createdAtMs'] ?? null));

            $meta = [
                'source' => 'convex',
                'sourceStatus' => $this->stringOrNull($row['status'] ?? null),
                'assistantMessageId' => $this->stringOrNull($row['assistantMessageId'] ?? null),
                'cancelRequested' => (bool) ($row['cancelRequested'] ?? false),
            ];

            if (! $dryRun) {
                DB::table('runs')->updateOrInsert(
                    ['id' => $runId],
                    [
                        'thread_id' => $threadId,
                        'user_id' => $ownerUserId,
                        'status' => $status,
                        'model_provider' => null,
                        'model' => null,
                        'usage' => null,
                        'meta' => json_encode($meta),
                        'error' => null,
                        'started_at' => $startedAt,
                        'completed_at' => $completedAt,
                        'created_at' => $createdAt,
                        'updated_at' => $updatedAt,
                    ],
                );
            }

            $runIdsImported[$runId] = true;
            $runsImported++;
        }

        $messagesImported = 0;

        foreach ($sourceMessages as $row) {
            $threadId = $this->stringOrNull($row['threadId'] ?? null);
            $messageId = $this->stringOrNull($row['messageId'] ?? null);
            $role = $this->stringOrNull($row['role'] ?? null);

            if (! $threadId || ! $messageId || ! in_array($role, ['user', 'assistant', 'system'], true)) {
                continue;
            }

            $ownerUserId = $threadOwnerUserIdByThreadId[$threadId] ?? null;
            if (! is_int($ownerUserId)) {
                continue;
            }

            $runId = $this->stringOrNull($row['runId'] ?? null);
            if (! $runId || ! isset($runIdsImported[$runId])) {
                $runId = null;
            }

            $content = (string) ($row['text'] ?? '');

            $createdAt = $this->fromMs($this->intOrNull($row['createdAtMs'] ?? null));
            $updatedAt = $this->fromMs($this->intOrNull($row['updatedAtMs'] ?? null) ?? $this->intOrNull($row['createdAtMs'] ?? null));

            $meta = [
                'source' => 'convex',
                'sourceStatus' => $this->stringOrNull($row['status'] ?? null),
                'sourceRunId' => $this->stringOrNull($row['runId'] ?? null),
            ];

            if (! $dryRun) {
                DB::table('messages')->updateOrInsert(
                    ['id' => $messageId],
                    [
                        'thread_id' => $threadId,
                        'run_id' => $runId,
                        'user_id' => $ownerUserId,
                        'role' => $role,
                        'content' => $content,
                        'meta' => json_encode($meta),
                        'created_at' => $createdAt,
                        'updated_at' => $updatedAt,
                    ],
                );

                DB::table('agent_conversation_messages')->updateOrInsert(
                    ['id' => $messageId],
                    [
                        'conversation_id' => $threadId,
                        'user_id' => $ownerUserId,
                        'agent' => 'autopilot',
                        'role' => $role,
                        'content' => $content,
                        'attachments' => '[]',
                        'tool_calls' => '[]',
                        'tool_results' => '[]',
                        'usage' => '{}',
                        'meta' => json_encode($meta),
                        'created_at' => $createdAt,
                        'updated_at' => $updatedAt,
                    ],
                );
            }

            $messagesImported++;
        }

        $eventsImported = 0;

        foreach ($sourceReceipts as $row) {
            $threadId = $this->stringOrNull($row['threadId'] ?? null);
            $runId = $this->stringOrNull($row['runId'] ?? null);

            if (! $threadId || ! $runId || ! Str::isUuid($runId)) {
                continue;
            }

            $ownerUserId = $threadOwnerUserIdByThreadId[$threadId] ?? null;
            if (! is_int($ownerUserId)) {
                continue;
            }

            $kind = $this->stringOrNull($row['kind'] ?? null) ?? 'unknown';
            $eventType = $this->mapReceiptEventType($kind);

            $payload = [
                'source' => 'convex_receipt',
                'kind' => $kind,
                'receiptId' => $this->stringOrNull($row['receiptId'] ?? null),
                'signatureId' => $this->stringOrNull($row['signatureId'] ?? null),
                'compiledId' => $this->stringOrNull($row['compiled_id'] ?? null),
                'json' => $row['json'] ?? null,
            ];

            $payloadJson = json_encode($payload);
            $createdAt = $this->fromMs($this->intOrNull($row['createdAtMs'] ?? null));

            if (! $dryRun) {
                $existsQuery = DB::table('run_events')
                    ->where('thread_id', $threadId)
                    ->where('run_id', $runId)
                    ->where('user_id', $ownerUserId)
                    ->where('type', $eventType)
                    ->where('created_at', $createdAt);

                if (DB::connection()->getDriverName() === 'pgsql') {
                    // Postgres does not support equality on json without explicit casts.
                    $existsQuery->whereRaw('payload::jsonb = ?::jsonb', [$payloadJson]);
                } else {
                    $existsQuery->where('payload', $payloadJson);
                }

                $exists = $existsQuery->exists();

                if (! $exists) {
                    DB::table('run_events')->insert([
                        'thread_id' => $threadId,
                        'run_id' => $runId,
                        'user_id' => $ownerUserId,
                        'type' => $eventType,
                        'payload' => $payloadJson,
                        'created_at' => $createdAt,
                    ]);
                }
            }

            $eventsImported++;
        }

        return [
            'source_users' => count($sourceUsers),
            'source_threads' => count($sourceThreads),
            'source_runs' => count($sourceRuns),
            'source_messages' => count($sourceMessages),
            'source_receipts' => count($sourceReceipts),
            'users_imported' => count($laravelUserIdByWorkosId),
            'threads_imported' => $threadsImported,
            'runs_imported' => $runsImported,
            'messages_imported' => $messagesImported,
            'events_imported' => $eventsImported,
        ];
    }

    private function truncateTargetTables(): void
    {
        DB::table('run_events')->truncate();
        DB::table('messages')->truncate();
        DB::table('runs')->truncate();
        DB::table('threads')->truncate();
        DB::table('agent_conversation_messages')->truncate();
        DB::table('agent_conversations')->truncate();
    }

    private function mapRunStatus(?string $status): string
    {
        return match ($status) {
            'streaming' => 'running',
            'final' => 'completed',
            'error' => 'failed',
            'canceled' => 'canceled',
            default => 'completed',
        };
    }

    private function mapReceiptEventType(string $kind): string
    {
        return match ($kind) {
            'model' => 'legacy_receipt_model',
            'tool' => 'legacy_receipt_tool',
            'dse.predict' => 'legacy_receipt_dse_predict',
            default => 'legacy_receipt',
        };
    }

    private function threadTitle(string $threadId, ?string $firstUserMessage): string
    {
        $seed = trim((string) $firstUserMessage);
        if ($seed === '') {
            return 'Conversation '.substr($threadId, 0, 8);
        }

        $normalized = preg_replace('/\s+/u', ' ', $seed) ?? $seed;

        return (string) Str::of(trim($normalized))->limit(120, '…');
    }

    private function preferredEmail(?string $email, string $workosId): string
    {
        $candidate = strtolower(trim((string) $email));
        if ($candidate !== '' && filter_var($candidate, FILTER_VALIDATE_EMAIL)) {
            return $candidate;
        }

        return 'migrated+'.substr(hash('sha256', $workosId), 0, 20).'@openagents.local';
    }

    /**
     * @param  array<string, string>  $usedEmails
     */
    private function uniqueEmail(string $email, string $workosId, array &$usedEmails, bool $dryRun): string
    {
        $candidate = strtolower($email);
        $attempt = 0;

        while (true) {
            $batchConflict = isset($usedEmails[$candidate]) && $usedEmails[$candidate] !== $workosId;
            $dbConflict = ! $dryRun
                ? DB::table('users')
                    ->where('email', $candidate)
                    ->where('workos_id', '!=', $workosId)
                    ->exists()
                : false;

            if (! $batchConflict && ! $dbConflict) {
                $usedEmails[$candidate] = $workosId;

                return $candidate;
            }

            $attempt++;
            $candidate = 'migrated+'.substr(hash('sha256', $workosId.'|'.$attempt), 0, 20).'@openagents.local';
        }
    }

    private function displayName(string $workosId, string $email): string
    {
        $fromEmail = trim(Str::before($email, '@'));
        if ($fromEmail !== '') {
            return Str::limit($fromEmail, 80, '');
        }

        return Str::limit($workosId, 80, '');
    }

    private function avatarUrl(string $email): string
    {
        $hash = md5(strtolower(trim($email)));

        return 'https://www.gravatar.com/avatar/'.$hash.'?d=identicon';
    }

    private function fromMs(?int $ms): Carbon
    {
        if (! is_int($ms) || $ms <= 0) {
            return now();
        }

        return Carbon::createFromTimestamp((int) floor($ms / 1000), 'UTC');
    }

    private function intOrNull(mixed $value): ?int
    {
        if (is_int($value)) {
            return $value;
        }

        if (is_numeric($value)) {
            return (int) $value;
        }

        return null;
    }

    private function stringOrNull(mixed $value): ?string
    {
        if (! is_string($value)) {
            return null;
        }

        $trimmed = trim($value);

        return $trimmed === '' ? null : $trimmed;
    }
}
