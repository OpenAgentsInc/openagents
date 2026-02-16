<?php

use App\Models\User;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;

it('redirects guests away from l402 pages', function () {
    $this->get('/l402')->assertRedirect('/login');
    $this->get('/l402/transactions')->assertRedirect('/login');
    $this->get('/l402/paywalls')->assertRedirect('/login');
    $this->get('/l402/settlements')->assertRedirect('/login');
    $this->get('/l402/deployments')->assertRedirect('/login');
});

it('renders l402 pages for authenticated users', function () {
    $user = User::factory()->create();
    $this->actingAs($user);

    $threadId = (string) Str::uuid7();
    $runId = (string) Str::uuid7();

    DB::table('threads')->insert([
        'id' => $threadId,
        'user_id' => $user->id,
        'title' => 'L402 test thread',
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    DB::table('runs')->insert([
        'id' => $runId,
        'thread_id' => $threadId,
        'user_id' => $user->id,
        'status' => 'completed',
        'model_provider' => 'test',
        'model' => 'test-model',
        'usage' => json_encode(['inputTokens' => 1, 'outputTokens' => 1]),
        'meta' => json_encode(['source' => 'test']),
        'error' => null,
        'started_at' => now(),
        'completed_at' => now(),
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    $receiptEventId = DB::table('run_events')->insertGetId([
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
            'proofReference' => 'preimage:abcd1234abcd1234',
            'responseStatusCode' => 200,
            'responseBodySha256' => str_repeat('a', 64),
            'tool_call_id' => 'toolcall_test',
        ]),
        'created_at' => now(),
    ]);

    DB::table('run_events')->insert([
        'thread_id' => $threadId,
        'run_id' => $runId,
        'user_id' => $user->id,
        'type' => 'l402_gateway_deployment',
        'payload' => json_encode([
            'deploymentId' => 'dep_test_1',
            'status' => 'applied',
            'configHash' => 'hash_test_1',
        ]),
        'created_at' => now(),
    ]);

    $this->get('/l402')->assertOk();
    $this->get('/l402/transactions')->assertOk();
    $this->get('/l402/paywalls')->assertOk();
    $this->get('/l402/settlements')->assertOk();
    $this->get('/l402/deployments')->assertOk();
    $this->get('/l402/transactions/'.$receiptEventId)->assertOk();
});

it('prevents accessing another users l402 transaction detail', function () {
    $owner = User::factory()->create();
    $viewer = User::factory()->create();

    $threadId = (string) Str::uuid7();
    $runId = (string) Str::uuid7();

    DB::table('threads')->insert([
        'id' => $threadId,
        'user_id' => $owner->id,
        'title' => 'Private L402 thread',
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    DB::table('runs')->insert([
        'id' => $runId,
        'thread_id' => $threadId,
        'user_id' => $owner->id,
        'status' => 'completed',
        'model_provider' => null,
        'model' => null,
        'usage' => null,
        'meta' => null,
        'error' => null,
        'started_at' => now(),
        'completed_at' => now(),
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    $eventId = DB::table('run_events')->insertGetId([
        'thread_id' => $threadId,
        'run_id' => $runId,
        'user_id' => $owner->id,
        'type' => 'l402_fetch_receipt',
        'payload' => json_encode([
            'status' => 'completed',
            'host' => 'sats4ai.com',
            'scope' => 'ep212.sats4ai',
            'paid' => true,
        ]),
        'created_at' => now(),
    ]);

    $this->actingAs($viewer)
        ->get('/l402/transactions/'.$eventId)
        ->assertNotFound();
});
