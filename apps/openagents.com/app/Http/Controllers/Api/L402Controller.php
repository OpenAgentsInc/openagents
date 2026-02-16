<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;

class L402Controller extends Controller
{
    public function wallet(Request $request): JsonResponse
    {
        $userId = (int) $request->user()->getAuthIdentifier();

        $receipts = $this->receiptQuery($userId)
            ->limit(200)
            ->get()
            ->map(fn ($row) => $this->mapReceiptRow($row))
            ->values();

        $totalPaidMsats = $receipts
            ->filter(fn (array $r) => $r['paid'] === true && is_int($r['amountMsats']))
            ->sum('amountMsats');

        $summary = [
            'totalAttempts' => $receipts->count(),
            'paidCount' => $receipts->where('paid', true)->count(),
            'cachedCount' => $receipts->filter(fn (array $r) => $r['status'] === 'cached' || $r['cacheStatus'] === 'hit')->count(),
            'blockedCount' => $receipts->where('status', 'blocked')->count(),
            'failedCount' => $receipts->where('status', 'failed')->count(),
            'totalPaidMsats' => $totalPaidMsats,
            'totalPaidSats' => $this->msatsToSats($totalPaidMsats),
        ];

        $lastPaid = $receipts->first(fn (array $r) => $r['paid'] === true);

        return response()->json([
            'data' => [
                'summary' => $summary,
                'lastPaid' => $lastPaid,
                'recent' => $receipts->take(20)->all(),
                'settings' => [
                    'allowlistHosts' => array_values(config('lightning.l402.allowlist_hosts', [])),
                    'invoicePayer' => (string) config('lightning.l402.invoice_payer', 'unknown'),
                    'credentialTtlSeconds' => (int) config('lightning.l402.credential_ttl_seconds', 0),
                    'paymentTimeoutMs' => (int) config('lightning.l402.payment_timeout_ms', 0),
                    'responseMaxBytes' => (int) config('lightning.l402.response_max_bytes', 0),
                    'responsePreviewBytes' => (int) config('lightning.l402.response_preview_bytes', 0),
                ],
            ],
        ]);
    }

    public function transactions(Request $request): JsonResponse
    {
        $userId = (int) $request->user()->getAuthIdentifier();

        $perPage = max(1, min(200, (int) $request->integer('per_page', 30)));
        $paginator = $this->receiptQuery($userId)->paginate($perPage);

        $rows = collect($paginator->items())
            ->map(fn ($row) => $this->mapReceiptRow($row))
            ->values()
            ->all();

        return response()->json([
            'data' => [
                'transactions' => $rows,
                'pagination' => [
                    'currentPage' => $paginator->currentPage(),
                    'lastPage' => $paginator->lastPage(),
                    'perPage' => $paginator->perPage(),
                    'total' => $paginator->total(),
                    'hasMorePages' => $paginator->hasMorePages(),
                ],
            ],
        ]);
    }

    public function transactionShow(Request $request, int $eventId): JsonResponse
    {
        $userId = (int) $request->user()->getAuthIdentifier();

        $row = $this->receiptQuery($userId)
            ->where('e.id', $eventId)
            ->first();

        if (! $row) {
            abort(404);
        }

        return response()->json([
            'data' => [
                'transaction' => $this->mapReceiptRow($row),
            ],
        ]);
    }

    public function paywalls(Request $request): JsonResponse
    {
        $userId = (int) $request->user()->getAuthIdentifier();

        $receipts = $this->receiptQuery($userId)
            ->limit(500)
            ->get()
            ->map(fn ($row) => $this->mapReceiptRow($row));

        $grouped = $receipts
            ->groupBy(fn (array $r) => ($r['host'] ?? 'unknown').'|'.($r['scope'] ?? ''))
            ->map(function ($items, $key) {
                [$host, $scope] = explode('|', (string) $key, 2);
                $sorted = collect($items)->sortByDesc('createdAt')->values();

                return [
                    'host' => $host,
                    'scope' => $scope,
                    'attempts' => $sorted->count(),
                    'paid' => $sorted->where('paid', true)->count(),
                    'cached' => $sorted->filter(fn (array $r) => $r['status'] === 'cached' || $r['cacheStatus'] === 'hit')->count(),
                    'blocked' => $sorted->where('status', 'blocked')->count(),
                    'failed' => $sorted->where('status', 'failed')->count(),
                    'totalPaidMsats' => $sorted
                        ->filter(fn (array $r) => $r['paid'] === true && is_int($r['amountMsats']))
                        ->sum('amountMsats'),
                    'totalPaidSats' => $this->msatsToSats(
                        $sorted
                            ->filter(fn (array $r) => $r['paid'] === true && is_int($r['amountMsats']))
                            ->sum('amountMsats')
                    ),
                    'lastAttemptAt' => $sorted->first()['createdAt'] ?? null,
                    'lastStatus' => $sorted->first()['status'] ?? 'unknown',
                ];
            })
            ->sortByDesc('lastAttemptAt')
            ->values();

        return response()->json([
            'data' => [
                'paywalls' => $grouped->all(),
                'summary' => [
                    'uniqueTargets' => $grouped->count(),
                    'totalAttempts' => $receipts->count(),
                    'totalPaidCount' => $receipts->where('paid', true)->count(),
                ],
            ],
        ]);
    }

    public function settlements(Request $request): JsonResponse
    {
        $userId = (int) $request->user()->getAuthIdentifier();

        $receipts = $this->receiptQuery($userId)
            ->limit(500)
            ->get()
            ->map(fn ($row) => $this->mapReceiptRow($row));

        $settlements = $receipts
            ->filter(fn (array $r) => $r['paid'] === true)
            ->values();

        $daily = $settlements
            ->groupBy(fn (array $r) => substr((string) ($r['createdAt'] ?? ''), 0, 10))
            ->map(function ($items, $date) {
                $totalMsats = collect($items)->filter(fn (array $r) => is_int($r['amountMsats']))->sum('amountMsats');

                return [
                    'date' => $date,
                    'count' => count($items),
                    'totalMsats' => $totalMsats,
                    'totalSats' => $this->msatsToSats($totalMsats),
                ];
            })
            ->sortByDesc('date')
            ->values();

        $totalMsats = $settlements->filter(fn (array $r) => is_int($r['amountMsats']))->sum('amountMsats');

        return response()->json([
            'data' => [
                'summary' => [
                    'settledCount' => $settlements->count(),
                    'totalMsats' => $totalMsats,
                    'totalSats' => $this->msatsToSats($totalMsats),
                    'latestSettlementAt' => $settlements->first()['createdAt'] ?? null,
                ],
                'daily' => $daily->all(),
                'settlements' => $settlements->take(100)->all(),
            ],
        ]);
    }

    public function deployments(Request $request): JsonResponse
    {
        $userId = (int) $request->user()->getAuthIdentifier();

        $events = DB::table('run_events as e')
            ->where('e.user_id', $userId)
            ->whereIn('e.type', [
                'l402_gateway_deployment',
                'l402_gateway_event',
                'l402_executor_heartbeat',
            ])
            ->orderByDesc('e.id')
            ->limit(100)
            ->get(['e.id', 'e.type', 'e.payload', 'e.created_at'])
            ->map(function ($row) {
                return [
                    'eventId' => (int) $row->id,
                    'type' => (string) $row->type,
                    'createdAt' => (string) $row->created_at,
                    'payload' => $this->decodePayload($row->payload),
                ];
            })
            ->values();

        return response()->json([
            'data' => [
                'deployments' => $events->all(),
                'configSnapshot' => [
                    'allowlistHosts' => array_values(config('lightning.l402.allowlist_hosts', [])),
                    'invoicePayer' => (string) config('lightning.l402.invoice_payer', 'unknown'),
                    'credentialTtlSeconds' => (int) config('lightning.l402.credential_ttl_seconds', 0),
                    'paymentTimeoutMs' => (int) config('lightning.l402.payment_timeout_ms', 0),
                    'demoPresets' => array_keys((array) config('lightning.demo_presets', [])),
                ],
            ],
        ]);
    }

    private function receiptQuery(int $userId)
    {
        return DB::table('run_events as e')
            ->leftJoin('threads as t', function ($join) {
                $join->on('t.id', '=', 'e.thread_id')
                    ->on('t.user_id', '=', 'e.user_id');
            })
            ->leftJoin('runs as r', function ($join) {
                $join->on('r.id', '=', 'e.run_id')
                    ->on('r.user_id', '=', 'e.user_id');
            })
            ->where('e.user_id', $userId)
            ->where('e.type', 'l402_fetch_receipt')
            ->select([
                'e.id',
                'e.thread_id',
                'e.run_id',
                'e.payload',
                'e.created_at',
                't.title as thread_title',
                'r.status as run_status',
                'r.started_at as run_started_at',
                'r.completed_at as run_completed_at',
            ])
            ->orderByDesc('e.id');
    }

    /**
     * @return array<string, mixed>
     */
    private function mapReceiptRow(object $row): array
    {
        $payload = $this->decodePayload($row->payload);

        $amountMsats = $this->toNullableInt($payload['amountMsats'] ?? null);
        $quotedAmountMsats = $this->toNullableInt($payload['quotedAmountMsats'] ?? null);
        $maxSpendMsats = $this->toNullableInt($payload['maxSpendMsats'] ?? null);

        return [
            'eventId' => (int) $row->id,
            'threadId' => (string) $row->thread_id,
            'threadTitle' => is_string($row->thread_title) ? $row->thread_title : 'Conversation',
            'runId' => (string) $row->run_id,
            'runStatus' => is_string($row->run_status) ? $row->run_status : null,
            'runStartedAt' => $row->run_started_at ? (string) $row->run_started_at : null,
            'runCompletedAt' => $row->run_completed_at ? (string) $row->run_completed_at : null,
            'createdAt' => (string) $row->created_at,
            'status' => $this->toNullableString($payload['status'] ?? null) ?? 'unknown',
            'host' => $this->toNullableString($payload['host'] ?? null) ?? 'unknown',
            'scope' => $this->toNullableString($payload['scope'] ?? null),
            'paid' => $this->toNullableBool($payload['paid'] ?? null) ?? false,
            'cacheHit' => $this->toNullableBool($payload['cacheHit'] ?? null) ?? false,
            'cacheStatus' => $this->toNullableString($payload['cacheStatus'] ?? null),
            'amountMsats' => $amountMsats,
            'amountSats' => $this->msatsToSats($amountMsats),
            'quotedAmountMsats' => $quotedAmountMsats,
            'quotedAmountSats' => $this->msatsToSats($quotedAmountMsats),
            'maxSpendMsats' => $maxSpendMsats,
            'maxSpendSats' => $this->msatsToSats($maxSpendMsats),
            'proofReference' => $this->toNullableString($payload['proofReference'] ?? null),
            'denyCode' => $this->toNullableString($payload['denyCode'] ?? null),
            'taskId' => $this->toNullableString($payload['taskId'] ?? null),
            'approvalRequired' => $this->toNullableBool($payload['approvalRequired'] ?? null) ?? false,
            'responseStatusCode' => $this->toNullableInt($payload['responseStatusCode'] ?? null),
            'responseBodySha256' => $this->toNullableString($payload['responseBodySha256'] ?? null),
            'toolCallId' => $this->toNullableString($payload['tool_call_id'] ?? null),
            'rawPayload' => $payload,
        ];
    }

    /**
     * @return array<string, mixed>
     */
    private function decodePayload(?string $payload): array
    {
        if (! is_string($payload) || $payload === '') {
            return [];
        }

        $decoded = json_decode($payload, true);

        return is_array($decoded) ? $decoded : [];
    }

    private function toNullableString(mixed $value): ?string
    {
        if (! is_string($value)) {
            return null;
        }

        $trimmed = trim($value);

        return $trimmed === '' ? null : $trimmed;
    }

    private function toNullableInt(mixed $value): ?int
    {
        if (is_int($value)) {
            return $value;
        }

        if (is_numeric($value)) {
            return (int) $value;
        }

        return null;
    }

    private function toNullableBool(mixed $value): ?bool
    {
        if (is_bool($value)) {
            return $value;
        }

        if ($value === 1 || $value === '1') {
            return true;
        }

        if ($value === 0 || $value === '0') {
            return false;
        }

        return null;
    }

    private function msatsToSats(?int $msats): ?float
    {
        if (! is_int($msats)) {
            return null;
        }

        return round($msats / 1000, 3);
    }
}
