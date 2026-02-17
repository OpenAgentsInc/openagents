<?php

use App\Models\User;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Http;

it('provisions wallet and executes agent payment endpoints with legacy aliases', function () {
    config()->set('lightning.spark_executor.base_url', 'https://spark-executor.test');
    config()->set('lightning.spark_executor.auth_token', null);

    $mnemonic = 'abandon ability able about above absent absorb abstract absurd abuse access accident';

    Http::fake([
        'https://spark-executor.test/wallets/create' => Http::response([
            'ok' => true,
            'result' => [
                'walletId' => 'oa-user-1',
                'mnemonic' => $mnemonic,
                'sparkAddress' => 'user1@spark.openagents.com',
                'lightningAddress' => 'user1@openagents.com',
                'identityPubkey' => 'pubkey_1',
                'balanceSats' => 0,
            ],
        ], 200),
        'https://spark-executor.test/wallets/status' => Http::response([
            'ok' => true,
            'status' => [
                'walletId' => 'oa-user-1',
                'sparkAddress' => 'user1@spark.openagents.com',
                'lightningAddress' => 'user1@openagents.com',
                'identityPubkey' => 'pubkey_1',
                'balanceSats' => 2100,
            ],
        ], 200),
        'https://spark-executor.test/wallets/create-invoice' => Http::response([
            'ok' => true,
            'result' => [
                'paymentRequest' => 'lnbc1testinvoicecreatedforwallet',
                'expiresAt' => now()->addHour()->toISOString(),
            ],
        ], 200),
        'https://spark-executor.test/wallets/pay-bolt11' => Http::response([
            'ok' => true,
            'result' => [
                'payment' => [
                    'status' => 'completed',
                    'paymentHash' => 'pay_hash_001',
                    'preimage' => str_repeat('b', 64),
                ],
            ],
        ], 200),
        'https://spark-executor.test/wallets/send-spark' => Http::response([
            'ok' => true,
            'result' => [
                'payment' => [
                    'status' => 'completed',
                    'paymentHash' => 'spark_transfer_hash_1',
                ],
            ],
        ], 200),
    ]);

    $user = User::factory()->create([
        'email' => 'agent-payments-api@openagents.com',
    ]);

    $token = $user->createToken('agent-payments')->plainTextToken;

    $this->withToken($token)
        ->getJson('/api/v1/agent-payments/wallet')
        ->assertNotFound();

    $this->withToken($token)
        ->postJson('/api/v1/agent-payments/wallet', [])
        ->assertOk()
        ->assertJsonPath('data.wallet.walletId', 'oa-user-1')
        ->assertJsonPath('data.wallet.sparkAddress', 'user1@spark.openagents.com')
        ->assertJsonPath('data.wallet.balanceSats', 2100);

    $this->withToken($token)
        ->getJson('/api/v1/agents/me/wallet')
        ->assertOk()
        ->assertJsonPath('data.wallet.walletId', 'oa-user-1');

    $this->withToken($token)
        ->getJson('/api/v1/agents/me/balance')
        ->assertOk()
        ->assertJsonPath('data.balanceSats', 2100);

    $this->withToken($token)
        ->postJson('/api/v1/payments/invoice', [
            'amountSats' => 123,
            'description' => 'Funding test',
        ])
        ->assertOk()
        ->assertJsonPath('data.invoice.paymentRequest', 'lnbc1testinvoicecreatedforwallet')
        ->assertJsonPath('data.invoice.amountSats', 123);

    $this->withToken($token)
        ->postJson('/api/v1/payments/pay', [
            'invoice' => 'lnbc1anotherinvoicepayloadforpaying',
            'maxAmountSats' => 100,
            'timeoutMs' => 12000,
            'host' => 'sats4ai.com',
        ])
        ->assertOk()
        ->assertJsonPath('data.payment.paymentId', 'pay_hash_001')
        ->assertJsonPath('data.payment.proofReference', 'preimage:'.substr(str_repeat('b', 64), 0, 16));

    $this->withToken($token)
        ->postJson('/api/v1/payments/send-spark', [
            'sparkAddress' => 'other@spark.openagents.com',
            'amountSats' => 21,
        ])
        ->assertOk()
        ->assertJsonPath('data.transfer.paymentId', 'spark_transfer_hash_1')
        ->assertJsonPath('data.transfer.amountSats', 21);

    $walletRow = DB::table('user_spark_wallets')->where('user_id', $user->id)->first();
    expect($walletRow)->not->toBeNull()
        ->and($walletRow->wallet_id)->toBe('oa-user-1')
        ->and($walletRow->mnemonic)->not->toBe($mnemonic);

    Http::assertSentCount(9);
});
