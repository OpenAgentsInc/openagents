<?php

namespace App\Lightning\Spark;

use App\Models\User;
use App\Models\UserSparkWallet;
use Illuminate\Support\Str;
use Throwable;

final class UserSparkWalletService
{
    public function __construct(private readonly SparkExecutorClient $sparkExecutor) {}

    public function walletForUser(User|int $user): ?UserSparkWallet
    {
        $userId = $this->resolveUserId($user);

        return UserSparkWallet::query()->where('user_id', $userId)->first();
    }

    public function ensureWalletForUser(User|int $user): UserSparkWallet
    {
        $userId = $this->resolveUserId($user);

        $existing = $this->walletForUser($userId);
        if ($existing) {
            return $existing;
        }

        return $this->provisionWalletForUser($userId, null);
    }

    public function importWalletForUser(User|int $user, string $mnemonic): UserSparkWallet
    {
        $userId = $this->resolveUserId($user);
        $normalizedMnemonic = $this->normalizeMnemonic($mnemonic);

        $existing = $this->walletForUser($userId);
        if (! $existing) {
            return $this->provisionWalletForUser($userId, $normalizedMnemonic);
        }

        $existing->fill([
            'mnemonic' => $normalizedMnemonic,
            'status' => 'syncing',
            'last_error' => null,
        ])->save();

        return $this->syncWallet($existing);
    }

    public function syncWallet(UserSparkWallet $wallet): UserSparkWallet
    {
        try {
            $status = $this->sparkExecutor->getWalletStatus($wallet->wallet_id, $wallet->mnemonic);

            $wallet->fill([
                'spark_address' => $this->nullableString($status['sparkAddress'] ?? null),
                'lightning_address' => $this->nullableString($status['lightningAddress'] ?? null),
                'identity_pubkey' => $this->nullableString($status['identityPubkey'] ?? null),
                'last_balance_sats' => $this->nullableInt($status['balanceSats'] ?? null),
                'status' => 'active',
                'last_error' => null,
                'meta' => $this->walletMetaFromStatus($status),
                'last_synced_at' => now(),
            ]);
            $wallet->save();
        } catch (Throwable $e) {
            $wallet->forceFill([
                'status' => 'error',
                'last_error' => $e->getMessage(),
            ])->save();

            throw $e;
        }

        return $wallet->fresh() ?? $wallet;
    }

    /**
     * @return array<string, mixed>
     */
    public function createInvoice(User|int $user, int $amountSats, ?string $description = null): array
    {
        $wallet = $this->ensureWalletForUser($user);

        $result = $this->sparkExecutor->createInvoice(
            walletId: $wallet->wallet_id,
            mnemonic: $wallet->mnemonic,
            amountSats: max(1, $amountSats),
            description: $description,
        );

        $this->syncWallet($wallet);

        return $result;
    }

    /**
     * @return array<string, mixed>
     */
    public function payBolt11(
        User|int $user,
        string $invoice,
        int $maxAmountMsats,
        int $timeoutMs,
        ?string $host = null,
    ): array {
        $wallet = $this->ensureWalletForUser($user);

        $result = $this->sparkExecutor->payBolt11(
            walletId: $wallet->wallet_id,
            mnemonic: $wallet->mnemonic,
            invoice: trim($invoice),
            maxAmountMsats: max(1000, $maxAmountMsats),
            timeoutMs: max(1000, $timeoutMs),
            host: $host,
        );

        $this->syncWallet($wallet);

        return $result;
    }

    /**
     * @return array<string, mixed>
     */
    public function sendToSpark(User|int $user, string $sparkAddress, int $amountSats, int $timeoutMs): array
    {
        $wallet = $this->ensureWalletForUser($user);

        $result = $this->sparkExecutor->sendToSpark(
            walletId: $wallet->wallet_id,
            mnemonic: $wallet->mnemonic,
            sparkAddress: trim($sparkAddress),
            amountSats: max(1, $amountSats),
            timeoutMs: max(1000, $timeoutMs),
        );

        $this->syncWallet($wallet);

        return $result;
    }

    private function provisionWalletForUser(int $userId, ?string $mnemonic): UserSparkWallet
    {
        $walletId = $this->walletIdForUser($userId);

        $created = $this->sparkExecutor->createWallet($walletId, $mnemonic);

        $resolvedMnemonic = $this->normalizeMnemonic((string) ($created['mnemonic'] ?? $mnemonic ?? ''));
        if ($resolvedMnemonic === '') {
            throw new SparkExecutorException('Spark executor did not return a mnemonic for wallet provisioning.');
        }

        $wallet = new UserSparkWallet;
        $wallet->forceFill([
            'user_id' => $userId,
            'wallet_id' => $walletId,
            'mnemonic' => $resolvedMnemonic,
            'spark_address' => $this->nullableString($created['sparkAddress'] ?? null),
            'lightning_address' => $this->nullableString($created['lightningAddress'] ?? null),
            'identity_pubkey' => $this->nullableString($created['identityPubkey'] ?? null),
            'last_balance_sats' => $this->nullableInt($created['balanceSats'] ?? null),
            'status' => 'active',
            'provider' => 'spark_executor',
            'last_error' => null,
            'meta' => $this->walletMetaFromStatus($created),
            'last_synced_at' => now(),
        ]);
        $wallet->save();

        return $wallet;
    }

    private function walletIdForUser(int $userId): string
    {
        $prefix = trim((string) config('lightning.agent_wallets.wallet_id_prefix', 'oa-user-'));
        if ($prefix === '') {
            $prefix = 'oa-user-';
        }

        return Str::slug($prefix.$userId, '-');
    }

    private function resolveUserId(User|int $user): int
    {
        if ($user instanceof User) {
            return (int) $user->getKey();
        }

        return (int) $user;
    }

    private function normalizeMnemonic(string $mnemonic): string
    {
        return trim(preg_replace('/\s+/', ' ', $mnemonic) ?? $mnemonic);
    }

    private function nullableString(mixed $value): ?string
    {
        return is_string($value) && trim($value) !== '' ? trim($value) : null;
    }

    private function nullableInt(mixed $value): ?int
    {
        if (is_int($value)) {
            return $value;
        }

        if (is_numeric($value)) {
            return (int) $value;
        }

        return null;
    }

    /**
     * @param  array<string, mixed>  $status
     * @return array<string, mixed>
     */
    private function walletMetaFromStatus(array $status): array
    {
        $meta = [];
        if (isset($status['paymentRequest']) && is_string($status['paymentRequest'])) {
            $meta['sparkPaymentRequest'] = $status['paymentRequest'];
        }
        if (isset($status['requestId']) && is_string($status['requestId'])) {
            $meta['requestId'] = $status['requestId'];
        }

        return $meta;
    }
}
