<?php

namespace App\Lightning\L402;

use Illuminate\Support\Carbon;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;

final class PendingL402ApprovalStore
{
    /**
     * @param  array<string, mixed>  $payload
     */
    public function create(array $payload): string
    {
        $taskId = (string) Str::uuid7();
        $ttlSeconds = max(30, (int) config('lightning.l402.approval_ttl_seconds', 600));

        DB::table('l402_pending_approvals')->insert([
            'id' => $taskId,
            'status' => 'pending',
            'payload' => json_encode($payload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE),
            'expires_at' => now()->addSeconds($ttlSeconds),
            'consumed_at' => null,
            'created_at' => now(),
            'updated_at' => now(),
        ]);

        return $taskId;
    }

    /**
     * @return array{status: 'consumed'|'missing'|'expired', payload?: array<string, mixed>}
     */
    public function consume(string $taskId): array
    {
        return DB::transaction(function () use ($taskId): array {
            $row = DB::table('l402_pending_approvals')
                ->where('id', $taskId)
                ->lockForUpdate()
                ->first();

            if (! $row) {
                return ['status' => 'missing'];
            }

            if (($row->status ?? null) !== 'pending') {
                return ['status' => 'missing'];
            }

            $expiresAt = is_string($row->expires_at) || $row->expires_at instanceof Carbon
                ? Carbon::parse($row->expires_at)
                : null;

            if ($expiresAt && $expiresAt->isPast()) {
                DB::table('l402_pending_approvals')
                    ->where('id', $taskId)
                    ->update([
                        'status' => 'expired',
                        'updated_at' => now(),
                    ]);

                return ['status' => 'expired'];
            }

            DB::table('l402_pending_approvals')
                ->where('id', $taskId)
                ->update([
                    'status' => 'consumed',
                    'consumed_at' => now(),
                    'updated_at' => now(),
                ]);

            $payload = [];
            if (is_string($row->payload) && $row->payload !== '') {
                $decoded = json_decode($row->payload, true);
                if (is_array($decoded)) {
                    $payload = $decoded;
                }
            }

            return [
                'status' => 'consumed',
                'payload' => $payload,
            ];
        });
    }

    public function pruneExpired(): void
    {
        DB::table('l402_pending_approvals')
            ->where('expires_at', '<=', now())
            ->whereIn('status', ['pending', 'expired'])
            ->delete();
    }
}
