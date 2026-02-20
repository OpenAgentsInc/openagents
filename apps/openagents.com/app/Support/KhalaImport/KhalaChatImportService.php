<?php

namespace App\Support\KhalaImport;

use App\Models\User;
use App\Services\WorkosUserLookupService;
use Illuminate\Support\Carbon;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;
use InvalidArgumentException;

final class KhalaChatImportService
{
    /**
     * @return array<string, int>
     */
    public function import(
        string $sourcePath,
        bool $replace,
        bool $dryRun,
        ?callable $logger = null,
        bool $resolveWorkosUsers = false,
        bool $importBlueprints = true,
    ): array {
        if (! file_exists($sourcePath)) {
            throw new InvalidArgumentException("Source path does not exist: [$sourcePath]");
        }

        $log = static function (string $message) use ($logger): void {
            if (is_callable($logger)) {
                $logger($message);
            }
        };

        $reader = new KhalaExportReader($sourcePath);

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
        /** @var array<int, array<string, mixed>> $sourceBlueprints */
        $sourceBlueprints = $importBlueprints ? $reader->readTable('blueprints') : [];

        $log(sprintf(
            'Loaded source rows: users=%d threads=%d runs=%d messages=%d receipts=%d blueprints=%d',
            count($sourceUsers),
            count($sourceThreads),
            count($sourceRuns),
            count($sourceMessages),
            count($sourceReceipts),
            count($sourceBlueprints),
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

        /** @var array<string, string> $usedEmails */
        $usedEmails = [];

        /** @var array<string, int> $laravelUserIdByWorkosId */
        $laravelUserIdByWorkosId = [];

        $workosLookup = $resolveWorkosUsers ? resolve(WorkosUserLookupService::class) : null;

        $usersResolvedViaWorkos = 0;
        $usersUnresolvedPlaceholder = 0;
        $usersEmailConflicts = 0;

        $syntheticId = 1;

        foreach ($userCandidates as $workosId => $candidate) {
            $sourceEmail = $this->stringOrNull($candidate['email'] ?? null);
            $resolved = null;

            if ($resolveWorkosUsers && $this->looksLikeWorkosUserId($workosId) && $workosLookup instanceof WorkosUserLookupService) {
                $resolved = $workosLookup->lookupByWorkosId($workosId);
            }

            $resolvedEmail = $this->stringOrNull($resolved['email'] ?? null);
            if (is_string($resolvedEmail) && $resolvedEmail !== '') {
                $usersResolvedViaWorkos++;
            }

            $candidateEmail = $this->preferredEmail($resolvedEmail ?? $sourceEmail, $workosId);
            $uniqueEmail = $this->uniqueEmail($candidateEmail, $workosId, $usedEmails, $dryRun);
            if ($uniqueEmail['hadConflict']) {
                $usersEmailConflicts++;
            }

            $email = $uniqueEmail['email'];

            if ($this->isPlaceholderEmail($email) && ! $this->isValidEmail($sourceEmail) && ! $this->isValidEmail($resolvedEmail)) {
                $usersUnresolvedPlaceholder++;
            }

            $resolvedName = $this->stringOrNull($resolved['name'] ?? null);
            $resolvedAvatar = $this->stringOrNull($resolved['avatar'] ?? null);

            $name = $this->displayName($workosId, $email, $resolvedName);
            $createdAt = $this->fromMs($candidate['createdAtMs'] ?? null);
            $avatar = $resolvedAvatar ?? $this->avatarUrl($email);

            if ($dryRun) {
                $laravelUserIdByWorkosId[$workosId] = $syntheticId++;
                $usedEmails[$email] = $workosId;

                continue;
            }

            $existing = DB::table('users')
                ->where('workos_id', $workosId)
                ->first(['id', 'name', 'email', 'avatar']);

            if ($existing) {
                $effectiveEmail = strtolower(trim((string) ($existing->email ?? '')));
                $updates = [];

                if ($this->shouldUpgradeEmail($effectiveEmail, $email)) {
                    $upgradeEmail = $this->uniqueEmail($email, $workosId, $usedEmails, false);
                    if ($upgradeEmail['hadConflict']) {
                        $usersEmailConflicts++;
                    }

                    $effectiveEmail = $upgradeEmail['email'];
                    $updates['email'] = $effectiveEmail;
                }

                if ($this->shouldUpgradeName((string) ($existing->name ?? ''), $workosId, $effectiveEmail, $resolvedName)) {
                    $updates['name'] = $name;
                }

                if ($this->shouldUpgradeAvatar((string) ($existing->avatar ?? ''), $resolvedAvatar)) {
                    $updates['avatar'] = $avatar;
                }

                if ($updates !== []) {
                    $updates['updated_at'] = now();
                    DB::table('users')->where('id', (int) $existing->id)->update($updates);
                }

                $laravelUserIdByWorkosId[$workosId] = (int) $existing->id;
                $usedEmails[$effectiveEmail] = $workosId;

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
            $usedEmails[$email] = $workosId;
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

        /**
         * @var array<string, array{updatedAtMs:int,blueprint:array<string,mixed>}>
         */
        $blueprintByThreadId = [];

        foreach ($sourceBlueprints as $row) {
            $threadId = $this->stringOrNull($row['threadId'] ?? null);
            if (! $threadId) {
                continue;
            }

            $blueprint = $this->extractBlueprintPayload($row);
            if (! is_array($blueprint)) {
                continue;
            }

            $updatedAtMs = $this->intOrNull($row['updatedAtMs'] ?? null)
                ?? $this->intOrNull($row['_creationTime'] ?? null)
                ?? 0;

            $existing = $blueprintByThreadId[$threadId] ?? null;

            if (! is_array($existing) || $updatedAtMs >= (int) ($existing['updatedAtMs'] ?? 0)) {
                $blueprintByThreadId[$threadId] = [
                    'updatedAtMs' => $updatedAtMs,
                    'blueprint' => $blueprint,
                ];
            }
        }

        $autopilotsCreated = 0;
        $autopilotProfilesUpserted = 0;
        $autopilotPoliciesUpserted = 0;
        $blueprintsMapped = 0;

        /** @var array<int, array{id:string,configVersion:int}> $autopilotByUserId */
        $autopilotByUserId = [];

        $ownerUserIds = array_values(array_unique(array_values($threadOwnerUserIdByThreadId)));

        foreach ($ownerUserIds as $ownerUserId) {
            $candidateBlueprint = $this->selectBlueprintForOwner(
                $ownerUserId,
                $threadOwnerUserIdByThreadId,
                $blueprintByThreadId,
            );

            $ownerUser = ! $dryRun
                ? DB::table('users')->where('id', $ownerUserId)->first(['id', 'name', 'email', 'handle'])
                : null;

            $autopilotContext = $this->ensureAutopilotForOwner(
                ownerUserId: $ownerUserId,
                ownerUser: $ownerUser,
                blueprintCandidate: $candidateBlueprint,
                dryRun: $dryRun,
                importBlueprints: $importBlueprints,
            );

            $autopilotByUserId[$ownerUserId] = [
                'id' => $autopilotContext['id'],
                'configVersion' => $autopilotContext['configVersion'],
            ];

            $autopilotsCreated += $autopilotContext['created'];
            $autopilotProfilesUpserted += $autopilotContext['profileUpserted'];
            $autopilotPoliciesUpserted += $autopilotContext['policyUpserted'];
            $blueprintsMapped += $autopilotContext['blueprintMapped'];
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
        $threadsLinkedBackfilled = 0;

        foreach ($threadRowsByThreadId as $threadId => $row) {
            $ownerUserId = $threadOwnerUserIdByThreadId[$threadId] ?? null;
            if (! is_int($ownerUserId)) {
                continue;
            }

            $autopilotContext = $autopilotByUserId[$ownerUserId] ?? null;
            $autopilotId = is_array($autopilotContext) ? (string) ($autopilotContext['id'] ?? '') : '';
            if ($autopilotId === '') {
                $autopilotId = null;
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
                        'autopilot_id' => $autopilotId,
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

                if (is_string($autopilotId) && $autopilotId !== '') {
                    $threadsLinkedBackfilled += DB::table('threads')
                        ->where('id', $threadId)
                        ->where('user_id', $ownerUserId)
                        ->whereNull('autopilot_id')
                        ->update(['autopilot_id' => $autopilotId]);
                }
            }

            $threadsImported++;
        }

        /** @var array<string, bool> $runIdsImported */
        $runIdsImported = [];
        $runsImported = 0;
        $runsLinkedBackfilled = 0;

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

            $autopilotContext = $autopilotByUserId[$ownerUserId] ?? null;
            $autopilotId = is_array($autopilotContext) ? (string) ($autopilotContext['id'] ?? '') : '';
            $autopilotConfigVersion = is_array($autopilotContext) ? (int) ($autopilotContext['configVersion'] ?? 1) : null;

            if ($autopilotId === '') {
                $autopilotId = null;
                $autopilotConfigVersion = null;
            }

            $status = $this->mapRunStatus($this->stringOrNull($row['status'] ?? null));
            $startedAt = $this->fromMs($this->intOrNull($row['createdAtMs'] ?? null));
            $completedAt = in_array($status, ['completed', 'failed', 'canceled'], true)
                ? $this->fromMs($this->intOrNull($row['updatedAtMs'] ?? null) ?? $this->intOrNull($row['createdAtMs'] ?? null))
                : null;
            $createdAt = $startedAt;
            $updatedAt = $this->fromMs($this->intOrNull($row['updatedAtMs'] ?? null) ?? $this->intOrNull($row['createdAtMs'] ?? null));

            $meta = [
                'source' => 'khala',
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
                        'autopilot_id' => $autopilotId,
                        'autopilot_config_version' => $autopilotConfigVersion,
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

                if (is_string($autopilotId) && $autopilotId !== '') {
                    $runsLinkedBackfilled += DB::table('runs')
                        ->where('id', $runId)
                        ->where('thread_id', $threadId)
                        ->where('user_id', $ownerUserId)
                        ->whereNull('autopilot_id')
                        ->update([
                            'autopilot_id' => $autopilotId,
                            'autopilot_config_version' => $autopilotConfigVersion,
                        ]);
                }
            }

            $runIdsImported[$runId] = true;
            $runsImported++;
        }

        $messagesImported = 0;
        $messagesLinkedBackfilled = 0;

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

            $autopilotContext = $autopilotByUserId[$ownerUserId] ?? null;
            $autopilotId = is_array($autopilotContext) ? (string) ($autopilotContext['id'] ?? '') : '';
            if ($autopilotId === '') {
                $autopilotId = null;
            }

            $runId = $this->stringOrNull($row['runId'] ?? null);
            if (! $runId || ! isset($runIdsImported[$runId])) {
                $runId = null;
            }

            $content = (string) ($row['text'] ?? '');

            $createdAt = $this->fromMs($this->intOrNull($row['createdAtMs'] ?? null));
            $updatedAt = $this->fromMs($this->intOrNull($row['updatedAtMs'] ?? null) ?? $this->intOrNull($row['createdAtMs'] ?? null));

            $meta = [
                'source' => 'khala',
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
                        'autopilot_id' => $autopilotId,
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

                if (is_string($autopilotId) && $autopilotId !== '') {
                    $messagesLinkedBackfilled += DB::table('messages')
                        ->where('id', $messageId)
                        ->where('thread_id', $threadId)
                        ->where('user_id', $ownerUserId)
                        ->whereNull('autopilot_id')
                        ->update([
                            'autopilot_id' => $autopilotId,
                        ]);
                }
            }

            $messagesImported++;
        }

        $eventsImported = 0;
        $eventsLinkedBackfilled = 0;

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

            $autopilotContext = $autopilotByUserId[$ownerUserId] ?? null;
            $autopilotId = is_array($autopilotContext) ? (string) ($autopilotContext['id'] ?? '') : '';
            if ($autopilotId === '') {
                $autopilotId = null;
            }

            $kind = $this->stringOrNull($row['kind'] ?? null) ?? 'unknown';
            $eventType = $this->mapReceiptEventType($kind);

            $payload = [
                'source' => 'khala_receipt',
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
                        'autopilot_id' => $autopilotId,
                        'actor_type' => is_string($autopilotId) ? 'autopilot' : 'user',
                        'actor_autopilot_id' => $autopilotId,
                        'type' => $eventType,
                        'payload' => $payloadJson,
                        'created_at' => $createdAt,
                    ]);
                }

                if (is_string($autopilotId) && $autopilotId !== '') {
                    $eventsLinkedBackfilled += DB::table('run_events')
                        ->where('thread_id', $threadId)
                        ->where('run_id', $runId)
                        ->where('user_id', $ownerUserId)
                        ->where('type', 'like', 'legacy_receipt%')
                        ->whereNull('autopilot_id')
                        ->update([
                            'autopilot_id' => $autopilotId,
                            'actor_type' => 'autopilot',
                            'actor_autopilot_id' => $autopilotId,
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
            'source_blueprints' => count($sourceBlueprints),
            'users_imported' => count($laravelUserIdByWorkosId),
            'users_resolved_via_workos' => $usersResolvedViaWorkos,
            'users_unresolved_placeholder' => $usersUnresolvedPlaceholder,
            'users_email_conflicts' => $usersEmailConflicts,
            'threads_imported' => $threadsImported,
            'runs_imported' => $runsImported,
            'messages_imported' => $messagesImported,
            'events_imported' => $eventsImported,
            'autopilots_created' => $autopilotsCreated,
            'autopilot_profiles_upserted' => $autopilotProfilesUpserted,
            'autopilot_policies_upserted' => $autopilotPoliciesUpserted,
            'blueprints_mapped' => $blueprintsMapped,
            'threads_linked_backfilled' => $threadsLinkedBackfilled,
            'runs_linked_backfilled' => $runsLinkedBackfilled,
            'messages_linked_backfilled' => $messagesLinkedBackfilled,
            'events_linked_backfilled' => $eventsLinkedBackfilled,
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
     * @return array{email:string,hadConflict:bool}
     */
    private function uniqueEmail(string $email, string $workosId, array &$usedEmails, bool $dryRun): array
    {
        $candidate = strtolower($email);
        $attempt = 0;
        $hadConflict = false;

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

                return [
                    'email' => $candidate,
                    'hadConflict' => $hadConflict,
                ];
            }

            $hadConflict = true;
            $attempt++;
            $candidate = 'migrated+'.substr(hash('sha256', $workosId.'|'.$attempt), 0, 20).'@openagents.local';
        }
    }

    private function displayName(string $workosId, string $email, ?string $resolvedName = null): string
    {
        $fromResolved = trim((string) $resolvedName);
        if ($fromResolved !== '') {
            return Str::limit($fromResolved, 80, '');
        }

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

    private function isValidEmail(?string $email): bool
    {
        if (! is_string($email)) {
            return false;
        }

        $candidate = strtolower(trim($email));
        if ($candidate === '') {
            return false;
        }

        return filter_var($candidate, FILTER_VALIDATE_EMAIL) !== false;
    }

    private function isPlaceholderEmail(string $email): bool
    {
        return str_ends_with(strtolower(trim($email)), '@openagents.local');
    }

    private function shouldUpgradeEmail(string $existingEmail, string $incomingEmail): bool
    {
        $existing = strtolower(trim($existingEmail));
        $incoming = strtolower(trim($incomingEmail));

        if ($incoming === '' || $existing === $incoming) {
            return false;
        }

        if (! $this->isValidEmail($existing) && $this->isValidEmail($incoming)) {
            return true;
        }

        if ($this->isPlaceholderEmail($existing) && ! $this->isPlaceholderEmail($incoming)) {
            return true;
        }

        return false;
    }

    private function shouldUpgradeName(string $existingName, string $workosId, string $existingEmail, ?string $resolvedName): bool
    {
        $name = trim($existingName);
        if ($name === '') {
            return true;
        }

        if (! is_string($resolvedName) || trim($resolvedName) === '') {
            return false;
        }

        $seedFromEmail = Str::before($existingEmail, '@');

        return $name === $workosId || $name === $seedFromEmail || str_starts_with($name, 'migrated+');
    }

    private function shouldUpgradeAvatar(string $existingAvatar, ?string $resolvedAvatar): bool
    {
        $incoming = trim((string) $resolvedAvatar);
        if ($incoming === '') {
            return false;
        }

        $existing = trim($existingAvatar);
        if ($existing === '') {
            return true;
        }

        return str_contains($existing, 'gravatar.com/avatar');
    }

    private function looksLikeWorkosUserId(string $workosId): bool
    {
        return preg_match('/^user_[a-zA-Z0-9_-]+$/', $workosId) === 1;
    }

    /**
     * @param  array<string, mixed>  $row
     * @return array<string, mixed>|null
     */
    private function extractBlueprintPayload(array $row): ?array
    {
        if (isset($row['blueprint']) && is_array($row['blueprint'])) {
            return $row['blueprint'];
        }

        if (isset($row['docs']) && is_array($row['docs'])) {
            return [
                'docs' => $row['docs'],
                'bootstrapState' => is_array($row['bootstrapState'] ?? null) ? $row['bootstrapState'] : null,
                'memory' => $row['memory'] ?? [],
            ];
        }

        return null;
    }

    /**
     * @param  array<string, int>  $threadOwnerUserIdByThreadId
     * @param  array<string, array{updatedAtMs:int,blueprint:array<string,mixed>}>  $blueprintByThreadId
     * @return array<string,mixed>|null
     */
    private function selectBlueprintForOwner(
        int $ownerUserId,
        array $threadOwnerUserIdByThreadId,
        array $blueprintByThreadId,
    ): ?array {
        $selected = null;
        $selectedUpdatedAt = PHP_INT_MIN;

        foreach ($threadOwnerUserIdByThreadId as $threadId => $candidateOwnerUserId) {
            if ($candidateOwnerUserId !== $ownerUserId) {
                continue;
            }

            $candidate = $blueprintByThreadId[$threadId] ?? null;
            if (! is_array($candidate)) {
                continue;
            }

            $updatedAt = (int) ($candidate['updatedAtMs'] ?? 0);
            if ($updatedAt >= $selectedUpdatedAt) {
                $selectedUpdatedAt = $updatedAt;
                $selected = $candidate;
            }
        }

        return $selected;
    }

    /**
     * @param  array<string,mixed>|null  $blueprintCandidate
     * @param  object{id:int,name?:string,email?:string,handle?:string}|null  $ownerUser
     * @return array{id:string,configVersion:int,created:int,profileUpserted:int,policyUpserted:int,blueprintMapped:int}
     */
    private function ensureAutopilotForOwner(
        int $ownerUserId,
        ?object $ownerUser,
        ?array $blueprintCandidate,
        bool $dryRun,
        bool $importBlueprints,
    ): array {
        $blueprint = is_array($blueprintCandidate['blueprint'] ?? null)
            ? $blueprintCandidate['blueprint']
            : null;

        $ownerName = trim((string) ($ownerUser?->name ?? ''));
        $ownerEmail = strtolower(trim((string) ($ownerUser?->email ?? '')));
        $ownerHandle = strtolower(trim((string) ($ownerUser?->handle ?? '')));

        $profileData = $this->profileDataFromBlueprint($blueprint, $ownerName);
        $policyData = $this->policyDataFromBlueprint($blueprint);
        $blueprintMapped = ($importBlueprints && is_array($blueprint)) ? 1 : 0;

        if ($dryRun) {
            return [
                'id' => 'dryrun-autopilot-'.$ownerUserId,
                'configVersion' => 1,
                'created' => 1,
                'profileUpserted' => 1,
                'policyUpserted' => 1,
                'blueprintMapped' => $blueprintMapped,
            ];
        }

        $autopilot = DB::table('autopilots')
            ->where('owner_user_id', $ownerUserId)
            ->orderBy('created_at')
            ->first(['id', 'config_version', 'display_name']);

        $created = 0;

        if (! $autopilot) {
            $handleSeed = $ownerHandle !== '' ? $ownerHandle.'-autopilot' : 'autopilot-'.$ownerUserId;
            $handle = $this->generateUniqueAutopilotHandle($handleSeed);

            $displayName = trim((string) ($profileData['owner_display_name'] ?? ''));
            if ($displayName === '') {
                $displayName = $ownerName !== '' ? $ownerName.' Autopilot' : 'Autopilot';
            }

            $id = (string) Str::uuid7();
            $now = now();

            $tagline = null;
            if ($importBlueprints) {
                $taglineSource = $this->stringOrNull($profileData['persona_summary'] ?? null);
                $tagline = is_string($taglineSource) ? Str::limit($taglineSource, 255, '') : null;
            }

            DB::table('autopilots')->insert([
                'id' => $id,
                'owner_user_id' => $ownerUserId,
                'handle' => $handle,
                'display_name' => Str::limit($displayName, 120, ''),
                'avatar' => null,
                'status' => 'active',
                'visibility' => 'private',
                'tagline' => $tagline,
                'config_version' => 1,
                'deleted_at' => null,
                'created_at' => $now,
                'updated_at' => $now,
            ]);

            $autopilot = DB::table('autopilots')->where('id', $id)->first(['id', 'config_version', 'display_name']);
            $created = 1;
        }

        $autopilotId = (string) $autopilot->id;
        $configVersion = max(1, (int) ($autopilot->config_version ?? 1));

        $profileUpserted = 0;
        $policyUpserted = 0;
        $configChanged = false;

        $existingProfile = DB::table('autopilot_profiles')->where('autopilot_id', $autopilotId)->first();
        $profilePayload = [
            'autopilot_id' => $autopilotId,
            'owner_display_name' => (string) ($profileData['owner_display_name'] ?? ($ownerName !== '' ? $ownerName : 'Autopilot')),
            'persona_summary' => $profileData['persona_summary'] ?? null,
            'autopilot_voice' => $profileData['autopilot_voice'] ?? null,
            'principles' => json_encode($profileData['principles'] ?? []),
            'preferences' => json_encode($profileData['preferences'] ?? []),
            'onboarding_answers' => json_encode($profileData['onboarding_answers'] ?? []),
            'schema_version' => 1,
            'updated_at' => now(),
        ];

        if (! $existingProfile) {
            DB::table('autopilot_profiles')->insert($profilePayload + [
                'created_at' => now(),
            ]);
            $profileUpserted = 1;
            $configChanged = true;
        } elseif ($this->rowNeedsUpdate($existingProfile, $profilePayload)) {
            DB::table('autopilot_profiles')->where('autopilot_id', $autopilotId)->update($profilePayload);
            $profileUpserted = 1;
            $configChanged = true;
        }

        $existingPolicy = DB::table('autopilot_policies')->where('autopilot_id', $autopilotId)->first();
        $policyPayload = [
            'autopilot_id' => $autopilotId,
            'model_provider' => null,
            'model' => null,
            'tool_allowlist' => json_encode([]),
            'tool_denylist' => json_encode([]),
            'l402_require_approval' => (bool) ($policyData['l402_require_approval'] ?? true),
            'l402_max_spend_msats_per_call' => null,
            'l402_max_spend_msats_per_day' => null,
            'l402_allowed_hosts' => json_encode($policyData['l402_allowed_hosts'] ?? []),
            'data_policy' => json_encode($policyData['data_policy'] ?? []),
            'updated_at' => now(),
        ];

        if (! $existingPolicy) {
            DB::table('autopilot_policies')->insert($policyPayload + [
                'created_at' => now(),
            ]);
            $policyUpserted = 1;
            $configChanged = true;
        } elseif ($this->rowNeedsUpdate($existingPolicy, $policyPayload)) {
            DB::table('autopilot_policies')->where('autopilot_id', $autopilotId)->update($policyPayload);
            $policyUpserted = 1;
            $configChanged = true;
        }

        if ($configChanged) {
            $configVersion++;
            DB::table('autopilots')->where('id', $autopilotId)->update([
                'config_version' => $configVersion,
                'updated_at' => now(),
            ]);
        }

        if ($created === 1 && $ownerEmail !== '' && filter_var($ownerEmail, FILTER_VALIDATE_EMAIL)) {
            DB::table('autopilots')->where('id', $autopilotId)->update([
                'avatar' => $this->avatarUrl($ownerEmail),
            ]);
        }

        return [
            'id' => $autopilotId,
            'configVersion' => $configVersion,
            'created' => $created,
            'profileUpserted' => $profileUpserted,
            'policyUpserted' => $policyUpserted,
            'blueprintMapped' => $blueprintMapped,
        ];
    }

    /**
     * @param  array<string,mixed>|null  $blueprint
     * @return array{owner_display_name:string,persona_summary:?string,autopilot_voice:?string,principles:array<int,mixed>,preferences:array<string,mixed>,onboarding_answers:array<string,mixed>}
     */
    private function profileDataFromBlueprint(?array $blueprint, string $ownerName): array
    {
        $docs = is_array($blueprint['docs'] ?? null) ? $blueprint['docs'] : [];
        $user = is_array($docs['user'] ?? null) ? $docs['user'] : [];
        $identity = is_array($docs['identity'] ?? null) ? $docs['identity'] : [];
        $character = is_array($docs['character'] ?? null) ? $docs['character'] : [];
        $rules = is_array($docs['rules'] ?? null) ? $docs['rules'] : [];
        $bootstrapState = is_array($blueprint['bootstrapState'] ?? null) ? $blueprint['bootstrapState'] : [];

        $ownerDisplayName = $this->stringOrNull($user['addressAs'] ?? null)
            ?? $this->stringOrNull($user['name'] ?? null)
            ?? ($ownerName !== '' ? $ownerName : 'Autopilot Owner');

        $autopilotVoice = $this->stringOrNull($identity['vibe'] ?? null)
            ?? $this->stringOrNull($character['vibe'] ?? null);

        $coreTruths = is_array($character['coreTruths'] ?? null)
            ? array_values(array_filter($character['coreTruths'], fn ($v) => is_string($v) && trim((string) $v) !== ''))
            : [];

        $boundaries = is_array($character['boundaries'] ?? null)
            ? array_values(array_filter($character['boundaries'], fn ($v) => is_string($v) && trim((string) $v) !== ''))
            : [];

        $personaParts = [];

        $identityName = $this->stringOrNull($identity['name'] ?? null);
        if (is_string($identityName)) {
            $personaParts[] = 'Identity: '.$identityName;
        }

        if (is_string($autopilotVoice)) {
            $personaParts[] = 'Voice: '.$autopilotVoice;
        }

        if ($coreTruths !== []) {
            $personaParts[] = 'Core truths: '.implode(' | ', $coreTruths);
        }

        if ($boundaries !== []) {
            $personaParts[] = 'Boundaries: '.implode(' | ', $boundaries);
        }

        $rulesBody = $this->stringOrNull($rules['body'] ?? null);
        if (is_string($rulesBody)) {
            $personaParts[] = 'Rules: '.preg_replace('/\s+/u', ' ', trim($rulesBody));
        }

        $personaSummary = $personaParts !== []
            ? Str::limit(implode(' || ', $personaParts), 2000, '')
            : null;

        return [
            'owner_display_name' => Str::limit($ownerDisplayName, 120, ''),
            'persona_summary' => $personaSummary,
            'autopilot_voice' => $autopilotVoice,
            'principles' => $coreTruths,
            'preferences' => [
                'user' => [
                    'name' => $this->stringOrNull($user['name'] ?? null),
                    'addressAs' => $this->stringOrNull($user['addressAs'] ?? null),
                    'pronouns' => $this->stringOrNull($user['pronouns'] ?? null),
                    'timeZone' => $this->stringOrNull($user['timeZone'] ?? null),
                    'context' => $this->stringOrNull($user['context'] ?? null),
                    'notes' => $this->stringOrNull($user['notes'] ?? null),
                ],
                'character' => [
                    'boundaries' => $boundaries,
                    'continuity' => $this->stringOrNull($character['continuity'] ?? null),
                    'vibe' => $this->stringOrNull($character['vibe'] ?? null),
                ],
                'identity' => [
                    'name' => $identityName,
                    'emoji' => $this->stringOrNull($identity['emoji'] ?? null),
                    'creature' => $this->stringOrNull($identity['creature'] ?? null),
                ],
            ],
            'onboarding_answers' => [
                'bootstrapState' => $bootstrapState,
                'user' => [
                    'name' => $this->stringOrNull($user['name'] ?? null),
                    'addressAs' => $this->stringOrNull($user['addressAs'] ?? null),
                    'pronouns' => $this->stringOrNull($user['pronouns'] ?? null),
                    'timeZone' => $this->stringOrNull($user['timeZone'] ?? null),
                ],
                'identity' => [
                    'name' => $identityName,
                    'vibe' => $this->stringOrNull($identity['vibe'] ?? null),
                ],
            ],
        ];
    }

    /**
     * @param  array<string,mixed>|null  $blueprint
     * @return array{l402_require_approval:bool,l402_allowed_hosts:array<int,string>,data_policy:array<string,mixed>}
     */
    private function policyDataFromBlueprint(?array $blueprint): array
    {
        $docs = is_array($blueprint['docs'] ?? null) ? $blueprint['docs'] : [];
        $rules = is_array($docs['rules'] ?? null) ? $docs['rules'] : [];
        $character = is_array($docs['character'] ?? null) ? $docs['character'] : [];

        $allowedHosts = config('lightning.l402.allowlist_hosts', []);
        if (! is_array($allowedHosts)) {
            $allowedHosts = [];
        }

        $normalizedHosts = [];
        foreach ($allowedHosts as $host) {
            if (! is_string($host)) {
                continue;
            }
            $value = strtolower(trim($host));
            if ($value === '') {
                continue;
            }
            $normalizedHosts[$value] = $value;
        }

        return [
            'l402_require_approval' => true,
            'l402_allowed_hosts' => array_values($normalizedHosts),
            'data_policy' => [
                'source' => 'khala_blueprint',
                'rules' => $this->stringOrNull($rules['body'] ?? null),
                'continuity' => $this->stringOrNull($character['continuity'] ?? null),
            ],
        ];
    }

    private function generateUniqueAutopilotHandle(string $seed): string
    {
        $base = User::normalizeHandleBase($seed);
        if ($base === '') {
            $base = 'autopilot';
        }

        $candidate = $base;
        $suffix = 1;

        while (DB::table('autopilots')->where('handle', $candidate)->exists()) {
            $suffixText = '-'.$suffix;
            $trimmed = substr($base, 0, max(1, 64 - strlen($suffixText)));
            $candidate = $trimmed.$suffixText;
            $suffix++;
        }

        return $candidate;
    }

    /**
     * @param  object  $existing
     * @param  array<string,mixed>  $payload
     */
    private function rowNeedsUpdate(object $existing, array $payload): bool
    {
        foreach ($payload as $key => $value) {
            if ($key === 'updated_at') {
                continue;
            }

            $existingValue = $existing->{$key} ?? null;

            if ($this->normalizeForCompare($existingValue) !== $this->normalizeForCompare($value)) {
                return true;
            }
        }

        return false;
    }

    private function normalizeForCompare(mixed $value): string
    {
        if ($value === null) {
            return 'null';
        }

        if (is_bool($value)) {
            return $value ? 'true' : 'false';
        }

        if (is_int($value) || is_float($value) || is_string($value)) {
            $stringValue = trim((string) $value);

            $decoded = json_decode($stringValue, true);
            if (json_last_error() === JSON_ERROR_NONE && is_array($decoded)) {
                return json_encode($this->canonicalize($decoded));
            }

            return $stringValue;
        }

        if (is_array($value)) {
            return json_encode($this->canonicalize($value));
        }

        return json_encode($this->canonicalize((array) $value));
    }

    private function canonicalize(mixed $value): mixed
    {
        if (! is_array($value)) {
            return $value;
        }

        if (array_is_list($value)) {
            return array_map(fn ($v) => $this->canonicalize($v), $value);
        }

        ksort($value);

        $out = [];
        foreach ($value as $k => $v) {
            $out[$k] = $this->canonicalize($v);
        }

        return $out;
    }
}
