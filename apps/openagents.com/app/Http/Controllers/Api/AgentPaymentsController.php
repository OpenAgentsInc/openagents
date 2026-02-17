<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Lightning\L402\Bolt11;
use App\Lightning\Spark\SparkExecutorException;
use App\Lightning\Spark\UserSparkWalletService;
use App\Models\UserSparkWallet;
use App\OpenApi\RequestBodies\CreateAgentInvoiceRequestBody;
use App\OpenApi\RequestBodies\PayAgentInvoiceRequestBody;
use App\OpenApi\RequestBodies\SendSparkPaymentRequestBody;
use App\OpenApi\RequestBodies\UpsertAgentWalletRequestBody;
use App\OpenApi\Responses\DataObjectResponse;
use App\OpenApi\Responses\NotFoundResponse;
use App\OpenApi\Responses\UnauthorizedResponse;
use App\OpenApi\Responses\ValidationErrorResponse;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Throwable;
use Vyuldashev\LaravelOpenApi\Attributes as OpenApi;

#[OpenApi\PathItem]
class AgentPaymentsController extends Controller
{
    public function __construct(private readonly UserSparkWalletService $wallets) {}

    /**
     * Get the authenticated user's Spark wallet metadata.
     */
    #[OpenApi\Operation(tags: ['Agent Payments'])]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    public function wallet(Request $request): JsonResponse
    {
        $user = $request->user();
        $wallet = $this->wallets->walletForUser((int) $user->getAuthIdentifier());

        if (! $wallet) {
            abort(404, 'wallet_not_found');
        }

        return response()->json([
            'data' => [
                'wallet' => $this->walletPayload($wallet),
            ],
        ]);
    }

    /**
     * Create or import wallet for the authenticated user.
     */
    #[OpenApi\Operation(tags: ['Agent Payments'])]
    #[OpenApi\RequestBody(factory: UpsertAgentWalletRequestBody::class)]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function upsertWallet(Request $request): JsonResponse
    {
        $validated = $request->validate([
            'mnemonic' => ['nullable', 'string', 'min:20'],
        ]);

        $userId = (int) $request->user()->getAuthIdentifier();
        $mnemonic = isset($validated['mnemonic']) ? trim((string) $validated['mnemonic']) : null;

        try {
            $wallet = is_string($mnemonic) && $mnemonic !== ''
                ? $this->wallets->importWalletForUser($userId, $mnemonic)
                : $this->wallets->ensureWalletForUser($userId);

            $wallet = $this->wallets->syncWallet($wallet);

            return response()->json([
                'data' => [
                    'wallet' => $this->walletPayload($wallet),
                    'action' => is_string($mnemonic) && $mnemonic !== '' ? 'imported' : 'ensured',
                ],
            ]);
        } catch (Throwable $e) {
            return $this->sparkError($e, 502);
        }
    }

    /**
     * Get current wallet balance for authenticated user.
     */
    #[OpenApi\Operation(tags: ['Agent Payments'])]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    public function balance(Request $request): JsonResponse
    {
        $userId = (int) $request->user()->getAuthIdentifier();
        $wallet = $this->wallets->walletForUser($userId);

        if (! $wallet) {
            abort(404, 'wallet_not_found');
        }

        try {
            $wallet = $this->wallets->syncWallet($wallet);

            return response()->json([
                'data' => [
                    'walletId' => $wallet->wallet_id,
                    'balanceSats' => $wallet->last_balance_sats,
                    'sparkAddress' => $wallet->spark_address,
                    'lightningAddress' => $wallet->lightning_address,
                    'lastSyncedAt' => optional($wallet->last_synced_at)?->toISOString(),
                ],
            ]);
        } catch (Throwable $e) {
            return $this->sparkError($e, 502);
        }
    }

    /**
     * Create a Lightning invoice from the authenticated user's wallet.
     */
    #[OpenApi\Operation(tags: ['Agent Payments'])]
    #[OpenApi\RequestBody(factory: CreateAgentInvoiceRequestBody::class)]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function createInvoice(Request $request): JsonResponse
    {
        $validated = $request->validate([
            'amountSats' => ['required', 'integer', 'min:1'],
            'description' => ['nullable', 'string', 'max:200'],
        ]);

        $userId = (int) $request->user()->getAuthIdentifier();

        try {
            $result = $this->wallets->createInvoice(
                user: $userId,
                amountSats: (int) $validated['amountSats'],
                description: isset($validated['description']) ? (string) $validated['description'] : null,
            );

            return response()->json([
                'data' => [
                    'invoice' => [
                        'paymentRequest' => $this->firstString($result, ['paymentRequest', 'invoice', 'bolt11']),
                        'amountSats' => (int) $validated['amountSats'],
                        'description' => $validated['description'] ?? null,
                        'expiresAt' => $this->firstString($result, ['expiresAt', 'expiryAt']),
                        'raw' => $result,
                    ],
                ],
            ]);
        } catch (Throwable $e) {
            return $this->sparkError($e, 502);
        }
    }

    /**
     * Pay a BOLT11 invoice from the authenticated user's wallet.
     */
    #[OpenApi\Operation(tags: ['Agent Payments'])]
    #[OpenApi\RequestBody(factory: PayAgentInvoiceRequestBody::class)]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function payInvoice(Request $request): JsonResponse
    {
        $validated = $request->validate([
            'invoice' => ['required', 'string', 'min:20'],
            'maxAmountSats' => ['nullable', 'integer', 'min:1'],
            'maxAmountMsats' => ['nullable', 'integer', 'min:1000'],
            'timeoutMs' => ['nullable', 'integer', 'min:1000', 'max:120000'],
            'host' => ['nullable', 'string', 'max:255'],
        ]);

        $invoice = trim((string) $validated['invoice']);
        $quotedMsats = Bolt11::amountMsats($invoice);

        $maxAmountMsats = null;
        if (isset($validated['maxAmountMsats'])) {
            $maxAmountMsats = (int) $validated['maxAmountMsats'];
        } elseif (isset($validated['maxAmountSats'])) {
            $maxAmountMsats = (int) $validated['maxAmountSats'] * 1000;
        } elseif (is_int($quotedMsats) && $quotedMsats > 0) {
            $maxAmountMsats = $quotedMsats;
        }

        if (! is_int($maxAmountMsats) || $maxAmountMsats <= 0) {
            return response()->json([
                'error' => [
                    'code' => 'max_amount_missing',
                    'message' => 'Unable to resolve max payment amount; provide maxAmountSats or maxAmountMsats.',
                ],
            ], 422);
        }

        $timeoutMs = isset($validated['timeoutMs'])
            ? (int) $validated['timeoutMs']
            : (int) config('lightning.l402.payment_timeout_ms', 12000);

        $userId = (int) $request->user()->getAuthIdentifier();

        try {
            $result = $this->wallets->payBolt11(
                user: $userId,
                invoice: $invoice,
                maxAmountMsats: $maxAmountMsats,
                timeoutMs: $timeoutMs,
                host: isset($validated['host']) ? (string) $validated['host'] : null,
            );

            $preimage = $this->firstString($result, ['preimage', 'paymentPreimage', 'payment.preimage', 'payment.paymentPreimage']);
            $paymentId = $this->firstString($result, ['paymentId', 'paymentHash', 'payment.paymentId', 'payment.paymentHash']);

            return response()->json([
                'data' => [
                    'payment' => [
                        'paymentId' => $paymentId,
                        'preimage' => $preimage,
                        'proofReference' => is_string($preimage) && $preimage !== '' ? 'preimage:'.substr($preimage, 0, 16) : null,
                        'quotedAmountMsats' => $quotedMsats,
                        'maxAmountMsats' => $maxAmountMsats,
                        'status' => $this->firstString($result, ['status', 'payment.status']) ?? 'completed',
                        'raw' => $result,
                    ],
                ],
            ]);
        } catch (Throwable $e) {
            return $this->sparkError($e, 502);
        }
    }

    /**
     * Send sats to another Spark address from authenticated user's wallet.
     */
    #[OpenApi\Operation(tags: ['Agent Payments'])]
    #[OpenApi\RequestBody(factory: SendSparkPaymentRequestBody::class)]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function sendSpark(Request $request): JsonResponse
    {
        $validated = $request->validate([
            'sparkAddress' => ['required', 'string', 'min:3', 'max:255'],
            'amountSats' => ['required', 'integer', 'min:1'],
            'timeoutMs' => ['nullable', 'integer', 'min:1000', 'max:120000'],
        ]);

        $timeoutMs = isset($validated['timeoutMs']) ? (int) $validated['timeoutMs'] : 12000;
        $userId = (int) $request->user()->getAuthIdentifier();

        try {
            $result = $this->wallets->sendToSpark(
                user: $userId,
                sparkAddress: (string) $validated['sparkAddress'],
                amountSats: (int) $validated['amountSats'],
                timeoutMs: $timeoutMs,
            );

            return response()->json([
                'data' => [
                    'transfer' => [
                        'sparkAddress' => (string) $validated['sparkAddress'],
                        'amountSats' => (int) $validated['amountSats'],
                        'status' => $this->firstString($result, ['status', 'payment.status']) ?? 'completed',
                        'paymentId' => $this->firstString($result, ['paymentId', 'paymentHash', 'payment.paymentId', 'payment.paymentHash']),
                        'raw' => $result,
                    ],
                ],
            ]);
        } catch (Throwable $e) {
            return $this->sparkError($e, 502);
        }
    }

    private function sparkError(Throwable $e, int $status): JsonResponse
    {
        $code = $e instanceof SparkExecutorException && is_string($e->codeName())
            ? $e->codeName()
            : 'spark_error';

        return response()->json([
            'error' => [
                'code' => $code,
                'message' => $e->getMessage(),
            ],
        ], $status);
    }

    /**
     * @return array<string, mixed>
     */
    private function walletPayload(UserSparkWallet $wallet): array
    {
        return [
            'id' => (int) $wallet->id,
            'walletId' => $wallet->wallet_id,
            'sparkAddress' => $wallet->spark_address,
            'lightningAddress' => $wallet->lightning_address,
            'identityPubkey' => $wallet->identity_pubkey,
            'balanceSats' => $wallet->last_balance_sats,
            'status' => $wallet->status,
            'provider' => $wallet->provider,
            'lastError' => $wallet->last_error,
            'lastSyncedAt' => optional($wallet->last_synced_at)?->toISOString(),
            'createdAt' => optional($wallet->created_at)?->toISOString(),
            'updatedAt' => optional($wallet->updated_at)?->toISOString(),
        ];
    }

    /**
     * @param  array<string, mixed>  $payload
     * @param  list<string>  $paths
     */
    private function firstString(array $payload, array $paths): ?string
    {
        foreach ($paths as $path) {
            $value = str_contains($path, '.') ? data_get($payload, $path) : ($payload[$path] ?? null);
            if (is_string($value) && trim($value) !== '') {
                return trim($value);
            }
        }

        return null;
    }
}
