<?php

use App\Lightning\L402\InvoicePayers\SparkWalletInvoicePayer;
use App\Lightning\Spark\UserSparkWalletService;
use App\Models\User;
use App\Models\UserSparkWallet;
use Illuminate\Support\Facades\Http;

it('requires authenticated user context for spark wallet payer', function () {
    $payer = new SparkWalletInvoicePayer(resolve(UserSparkWalletService::class));

    expect(fn () => $payer->payBolt11('lnbc1exampleinvoice', 12000, []))
        ->toThrow(\RuntimeException::class, 'requires authenticated user context');
});

it('delegates invoice payment to user spark wallet service', function () {
    config()->set('lightning.spark_executor.base_url', 'https://spark-executor.test');
    config()->set('lightning.spark_executor.auth_token', null);

    Http::fake([
        'https://spark-executor.test/wallets/pay-bolt11' => Http::response([
            'ok' => true,
            'result' => [
                'payment' => [
                    'status' => 'completed',
                    'paymentHash' => 'pay_hash_123',
                    'preimage' => str_repeat('a', 64),
                ],
            ],
        ], 200),
        'https://spark-executor.test/wallets/status' => Http::response([
            'ok' => true,
            'status' => [
                'walletId' => 'oa-user-1',
                'sparkAddress' => 'user1@spark.openagents.com',
                'lightningAddress' => 'user1@openagents.com',
                'identityPubkey' => 'pubkey_1',
                'balanceSats' => 42,
            ],
        ], 200),
    ]);

    $user = User::factory()->create();

    UserSparkWallet::query()->create([
        'user_id' => $user->id,
        'wallet_id' => 'oa-user-1',
        'mnemonic' => 'abandon ability able about above absent absorb abstract absurd abuse access accident',
        'spark_address' => 'user1@spark.openagents.com',
        'lightning_address' => 'user1@openagents.com',
        'identity_pubkey' => 'pubkey_1',
        'last_balance_sats' => 0,
        'status' => 'active',
        'provider' => 'spark_executor',
        'last_error' => null,
        'meta' => [],
        'last_synced_at' => now(),
    ]);

    $payer = new SparkWalletInvoicePayer(resolve(UserSparkWalletService::class));

    $result = $payer->payBolt11('lnbc1testinvoicepayload', 12000, [
        'userId' => (int) $user->id,
        'maxSpendMsats' => 100000,
        'host' => 'sats4ai.com',
    ]);

    expect($result->preimage)->toBe(str_repeat('a', 64))
        ->and($result->paymentId)->toBe('pay_hash_123');

    Http::assertSentCount(2);
});
