<?php

use App\Models\User;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;

function seedL402ThreadAndRun(int $userId, ?string $autopilotId = null): array
{
    $threadId = (string) Str::uuid7();
    $runId = (string) Str::uuid7();

    DB::table('threads')->insert([
        'id' => $threadId,
        'user_id' => $userId,
        'autopilot_id' => $autopilotId,
        'title' => 'L402 API thread',
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    DB::table('runs')->insert([
        'id' => $runId,
        'thread_id' => $threadId,
        'user_id' => $userId,
        'autopilot_id' => $autopilotId,
        'autopilot_config_version' => 1,
        'status' => 'completed',
        'model_provider' => 'openrouter',
        'model' => 'moonshotai/kimi-k2.5',
        'usage' => json_encode(['inputTokens' => 10, 'outputTokens' => 12]),
        'meta' => json_encode(['source' => 'test']),
        'error' => null,
        'started_at' => now(),
        'completed_at' => now(),
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    return [$threadId, $runId];
}

function seedL402ReceiptEvent(int $userId, string $threadId, string $runId, array $payload, ?string $autopilotId = null): int
{
    return DB::table('run_events')->insertGetId([
        'thread_id' => $threadId,
        'run_id' => $runId,
        'user_id' => $userId,
        'autopilot_id' => $autopilotId,
        'type' => 'l402_fetch_receipt',
        'payload' => json_encode($payload),
        'created_at' => now(),
    ]);
}

function seedL402DeploymentEvent(int $userId, string $threadId, string $runId, string $type, array $payload, ?string $autopilotId = null): int
{
    return DB::table('run_events')->insertGetId([
        'thread_id' => $threadId,
        'run_id' => $runId,
        'user_id' => $userId,
        'autopilot_id' => $autopilotId,
        'type' => $type,
        'payload' => json_encode($payload),
        'created_at' => now(),
    ]);
}

function seedAutopilot(int $ownerUserId, string $handle): string
{
    $autopilotId = (string) Str::uuid7();

    DB::table('autopilots')->insert([
        'id' => $autopilotId,
        'owner_user_id' => $ownerUserId,
        'handle' => $handle,
        'display_name' => strtoupper($handle),
        'status' => 'active',
        'visibility' => 'private',
        'tagline' => null,
        'config_version' => 1,
        'deleted_at' => null,
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    return $autopilotId;
}

it('exposes l402 wallet and receipt endpoints via api', function () {
    $user = User::factory()->create([
        'email' => 'l402-user@openagents.com',
    ]);

    $token = $user->createToken('l402-api')->plainTextToken;

    [$threadId, $runId] = seedL402ThreadAndRun($user->id);

    $eventId = seedL402ReceiptEvent($user->id, $threadId, $runId, [
        'status' => 'completed',
        'host' => 'sats4ai.com',
        'scope' => 'ep212.sats4ai',
        'paid' => true,
        'cacheHit' => false,
        'cacheStatus' => 'miss',
        'amountMsats' => 42000,
        'quotedAmountMsats' => 42000,
        'maxSpendMsats' => 100000,
        'proofReference' => 'preimage:abcdef1234567890',
        'responseStatusCode' => 200,
        'responseBodySha256' => str_repeat('a', 64),
        'tool_call_id' => 'toolcall_l402_test',
    ]);

    seedL402DeploymentEvent($user->id, $threadId, $runId, 'l402_gateway_deployment', [
        'deploymentId' => 'dep_1',
        'status' => 'applied',
    ]);

    $this->withToken($token)
        ->getJson('/api/l402/wallet')
        ->assertOk()
        ->assertJsonPath('data.summary.totalAttempts', 1)
        ->assertJsonPath('data.summary.paidCount', 1)
        ->assertJsonPath('data.summary.totalPaidMsats', 42000)
        ->assertJsonPath('data.filter.autopilot', null);

    $this->withToken($token)
        ->getJson('/api/l402/transactions')
        ->assertOk()
        ->assertJsonPath('data.pagination.total', 1)
        ->assertJsonPath('data.transactions.0.eventId', $eventId)
        ->assertJsonPath('data.transactions.0.paid', true)
        ->assertJsonPath('data.filter.autopilot', null);

    $this->withToken($token)
        ->getJson('/api/l402/transactions/'.$eventId)
        ->assertOk()
        ->assertJsonPath('data.transaction.eventId', $eventId)
        ->assertJsonPath('data.transaction.host', 'sats4ai.com');

    $this->withToken($token)
        ->getJson('/api/l402/paywalls')
        ->assertOk()
        ->assertJsonPath('data.summary.uniqueTargets', 1)
        ->assertJsonPath('data.summary.totalPaidCount', 1)
        ->assertJsonPath('data.filter.autopilot', null);

    $this->withToken($token)
        ->getJson('/api/l402/settlements')
        ->assertOk()
        ->assertJsonPath('data.summary.settledCount', 1)
        ->assertJsonPath('data.summary.totalMsats', 42000)
        ->assertJsonPath('data.filter.autopilot', null);

    $this->withToken($token)
        ->getJson('/api/l402/deployments')
        ->assertOk()
        ->assertJsonPath('data.deployments.0.type', 'l402_gateway_deployment')
        ->assertJsonPath('data.filter.autopilot', null);
});

it('supports autopilot scoped filtering across l402 analytics endpoints', function () {
    $user = User::factory()->create([
        'email' => 'l402-filter-user@openagents.com',
    ]);

    $token = $user->createToken('l402-filter-api')->plainTextToken;

    $autopilotA = seedAutopilot($user->id, 'ep212-a');
    $autopilotB = seedAutopilot($user->id, 'ep212-b');

    [$threadA, $runA] = seedL402ThreadAndRun($user->id, $autopilotA);
    [$threadB, $runB] = seedL402ThreadAndRun($user->id, $autopilotB);
    [$threadGlobal, $runGlobal] = seedL402ThreadAndRun($user->id);

    $eventA = seedL402ReceiptEvent($user->id, $threadA, $runA, [
        'status' => 'completed',
        'host' => 'alpha.example.com',
        'scope' => 'ep212.alpha',
        'paid' => true,
        'cacheHit' => false,
        'cacheStatus' => 'miss',
        'amountMsats' => 11111,
        'quotedAmountMsats' => 11111,
        'maxSpendMsats' => 100000,
        'proofReference' => 'preimage:alpha',
        'responseStatusCode' => 200,
        'responseBodySha256' => str_repeat('b', 64),
        'tool_call_id' => 'toolcall_alpha',
    ], $autopilotA);

    seedL402ReceiptEvent($user->id, $threadB, $runB, [
        'status' => 'completed',
        'host' => 'beta.example.com',
        'scope' => 'ep212.beta',
        'paid' => true,
        'cacheHit' => false,
        'cacheStatus' => 'miss',
        'amountMsats' => 22222,
        'quotedAmountMsats' => 22222,
        'maxSpendMsats' => 100000,
        'proofReference' => 'preimage:beta',
        'responseStatusCode' => 200,
        'responseBodySha256' => str_repeat('c', 64),
        'tool_call_id' => 'toolcall_beta',
    ], $autopilotB);

    seedL402ReceiptEvent($user->id, $threadGlobal, $runGlobal, [
        'status' => 'blocked',
        'host' => 'global.example.com',
        'scope' => 'ep212.global',
        'paid' => false,
        'cacheHit' => false,
        'cacheStatus' => 'miss',
        'amountMsats' => null,
        'quotedAmountMsats' => 33333,
        'maxSpendMsats' => 50000,
        'denyCode' => 'policy.maxSpendExceeded',
        'tool_call_id' => 'toolcall_global',
    ]);

    seedL402DeploymentEvent($user->id, $threadA, $runA, 'l402_gateway_deployment', [
        'deploymentId' => 'dep_a',
        'status' => 'applied',
    ], $autopilotA);

    seedL402DeploymentEvent($user->id, $threadB, $runB, 'l402_gateway_event', [
        'event' => 'reconcile',
    ], $autopilotB);

    $this->withToken($token)
        ->getJson('/api/l402/wallet')
        ->assertOk()
        ->assertJsonPath('data.summary.totalAttempts', 3)
        ->assertJsonPath('data.summary.paidCount', 2)
        ->assertJsonPath('data.filter.autopilot', null);

    $this->withToken($token)
        ->getJson('/api/l402/wallet?autopilot='.$autopilotA)
        ->assertOk()
        ->assertJsonPath('data.summary.totalAttempts', 1)
        ->assertJsonPath('data.summary.totalPaidMsats', 11111)
        ->assertJsonPath('data.filter.autopilot.id', $autopilotA)
        ->assertJsonPath('data.filter.autopilot.handle', 'ep212-a');

    $this->withToken($token)
        ->getJson('/api/l402/transactions?autopilot=ep212-a')
        ->assertOk()
        ->assertJsonPath('data.pagination.total', 1)
        ->assertJsonPath('data.transactions.0.eventId', $eventA)
        ->assertJsonPath('data.transactions.0.host', 'alpha.example.com')
        ->assertJsonPath('data.filter.autopilot.id', $autopilotA)
        ->assertJsonPath('data.filter.autopilot.handle', 'ep212-a');

    $this->withToken($token)
        ->getJson('/api/l402/paywalls?autopilot=ep212-a')
        ->assertOk()
        ->assertJsonPath('data.summary.totalAttempts', 1)
        ->assertJsonPath('data.summary.uniqueTargets', 1)
        ->assertJsonPath('data.paywalls.0.host', 'alpha.example.com')
        ->assertJsonPath('data.filter.autopilot.id', $autopilotA);

    $this->withToken($token)
        ->getJson('/api/l402/settlements?autopilot=ep212-a')
        ->assertOk()
        ->assertJsonPath('data.summary.settledCount', 1)
        ->assertJsonPath('data.summary.totalMsats', 11111)
        ->assertJsonPath('data.filter.autopilot.handle', 'ep212-a');

    $this->withToken($token)
        ->getJson('/api/l402/deployments?autopilot=ep212-a')
        ->assertOk()
        ->assertJsonPath('data.deployments.0.payload.deploymentId', 'dep_a')
        ->assertJsonPath('data.filter.autopilot.id', $autopilotA);
});

it('rejects unauthorized autopilot filters and missing autopilot filters', function () {
    $owner = User::factory()->create([
        'email' => 'l402-filter-owner@openagents.com',
    ]);
    $intruder = User::factory()->create([
        'email' => 'l402-filter-intruder@openagents.com',
    ]);

    $ownerToken = $owner->createToken('owner-l402-filter-api')->plainTextToken;
    $intruderToken = $intruder->createToken('intruder-l402-filter-api')->plainTextToken;

    $ownerAutopilot = seedAutopilot($owner->id, 'owner-ep212');

    $this->withToken($intruderToken)
        ->getJson('/api/l402/wallet?autopilot='.$ownerAutopilot)
        ->assertForbidden();

    $this->withToken($intruderToken)
        ->getJson('/api/l402/transactions?autopilot=owner-ep212')
        ->assertForbidden();

    $this->withToken($ownerToken)
        ->getJson('/api/l402/settlements?autopilot=missing-bot')
        ->assertNotFound();
});

it('returns spark wallet snapshot fields used by the sidebar card', function () {
    $user = User::factory()->create([
        'email' => 'l402-wallet-snapshot@openagents.com',
    ]);

    DB::table('user_spark_wallets')->insert([
        'user_id' => $user->id,
        'wallet_id' => 'wallet_sidebar_snapshot_1',
        'mnemonic' => encrypt('abandon ability able about above absent absorb abstract absurd abuse access accident'),
        'spark_address' => 'spark:sidebar-demo',
        'lightning_address' => 'sidebar-demo@openagents.com',
        'identity_pubkey' => '02'.str_repeat('a', 64),
        'last_balance_sats' => 12345,
        'status' => 'active',
        'provider' => 'spark_executor',
        'last_error' => null,
        'meta' => json_encode(['source' => 'test']),
        'last_synced_at' => now(),
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    $token = $user->createToken('l402-wallet-card')->plainTextToken;

    $this->withToken($token)
        ->getJson('/api/l402/wallet')
        ->assertOk()
        ->assertJsonPath('data.sparkWallet.walletId', 'wallet_sidebar_snapshot_1')
        ->assertJsonPath('data.sparkWallet.sparkAddress', 'spark:sidebar-demo')
        ->assertJsonPath('data.sparkWallet.lightningAddress', 'sidebar-demo@openagents.com')
        ->assertJsonPath('data.sparkWallet.balanceSats', 12345)
        ->assertJsonPath('data.sparkWallet.status', 'active')
        ->assertJsonPath('data.settings.invoicePayer', 'spark_wallet')
        ->assertJsonPath('data.summary.totalAttempts', 0)
        ->assertJsonPath('data.filter.autopilot', null);
});
