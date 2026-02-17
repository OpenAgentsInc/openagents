<?php

use App\Models\User;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;

it('exposes l402 wallet and receipt endpoints via api', function () {
    $user = User::factory()->create([
        'email' => 'l402-user@openagents.com',
    ]);

    $token = $user->createToken('l402-api')->plainTextToken;

    $threadId = (string) Str::uuid7();
    $runId = (string) Str::uuid7();

    DB::table('threads')->insert([
        'id' => $threadId,
        'user_id' => $user->id,
        'title' => 'L402 API thread',
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    DB::table('runs')->insert([
        'id' => $runId,
        'thread_id' => $threadId,
        'user_id' => $user->id,
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

    $eventId = DB::table('run_events')->insertGetId([
        'thread_id' => $threadId,
        'run_id' => $runId,
        'user_id' => $user->id,
        'type' => 'l402_fetch_receipt',
        'payload' => json_encode([
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
        ]),
        'created_at' => now(),
    ]);

    DB::table('run_events')->insert([
        'thread_id' => $threadId,
        'run_id' => $runId,
        'user_id' => $user->id,
        'type' => 'l402_gateway_deployment',
        'payload' => json_encode([
            'deploymentId' => 'dep_1',
            'status' => 'applied',
        ]),
        'created_at' => now(),
    ]);

    $this->withToken($token)
        ->getJson('/api/l402/wallet')
        ->assertOk()
        ->assertJsonPath('data.summary.totalAttempts', 1)
        ->assertJsonPath('data.summary.paidCount', 1)
        ->assertJsonPath('data.summary.totalPaidMsats', 42000);

    $this->withToken($token)
        ->getJson('/api/l402/transactions')
        ->assertOk()
        ->assertJsonPath('data.pagination.total', 1)
        ->assertJsonPath('data.transactions.0.eventId', $eventId)
        ->assertJsonPath('data.transactions.0.paid', true);

    $this->withToken($token)
        ->getJson('/api/l402/transactions/'.$eventId)
        ->assertOk()
        ->assertJsonPath('data.transaction.eventId', $eventId)
        ->assertJsonPath('data.transaction.host', 'sats4ai.com');

    $this->withToken($token)
        ->getJson('/api/l402/paywalls')
        ->assertOk()
        ->assertJsonPath('data.summary.uniqueTargets', 1)
        ->assertJsonPath('data.summary.totalPaidCount', 1);

    $this->withToken($token)
        ->getJson('/api/l402/settlements')
        ->assertOk()
        ->assertJsonPath('data.summary.settledCount', 1)
        ->assertJsonPath('data.summary.totalMsats', 42000);

    $this->withToken($token)
        ->getJson('/api/l402/deployments')
        ->assertOk()
        ->assertJsonPath('data.deployments.0.type', 'l402_gateway_deployment');
});
