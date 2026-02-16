<?php

use App\AI\RunOrchestrator;
use App\Models\User;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;
use Laravel\Ai\Responses\Data\ToolCall as ToolCallData;
use Laravel\Ai\Responses\Data\ToolResult as ToolResultData;
use Laravel\Ai\Responses\Data\Usage;
use Laravel\Ai\Responses\StreamableAgentResponse;
use Laravel\Ai\Streaming\Events\StreamEnd;
use Laravel\Ai\Streaming\Events\StreamStart;
use Laravel\Ai\Streaming\Events\TextDelta;
use Laravel\Ai\Streaming\Events\ToolCall;
use Laravel\Ai\Streaming\Events\ToolResult;

test('l402 tool results emit an l402_fetch_receipt run_event with key metadata', function () {
    $user = User::factory()->create();

    $conversationId = (string) Str::uuid7();

    DB::table('agent_conversations')->insert([
        'id' => $conversationId,
        'user_id' => $user->id,
        'title' => 'L402 receipt test',
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    $toolCallId = 'toolcall_l402_1';
    $messageId = 'msg_l402_1';

    $toolArgs = [
        'url' => 'https://sats4ai.com/api/l402/text-generation',
        'method' => 'POST',
        'maxSpendSats' => 100,
        'scope' => 'ep212.sats4ai',
    ];

    $toolResult = [
        'status' => 'completed',
        'host' => 'sats4ai.com',
        'scope' => 'ep212.sats4ai',
        'paid' => true,
        'cacheHit' => false,
        'cacheStatus' => 'miss',
        'maxSpendMsats' => 100000,
        'quotedAmountMsats' => 42000,
        'amountMsats' => 42000,
        'proofReference' => 'preimage:deadbeefdeadbeef',
        'responseStatusCode' => 200,
        'responseBodySha256' => str_repeat('a', 64),
    ];

    $streamable = new StreamableAgentResponse('invocation_l402_1', function () use ($messageId, $toolCallId, $toolArgs, $toolResult) {
        yield (new StreamStart('start_l402', 'fake', 'fake-model', 1000))->withInvocationId('invocation_l402_1');

        yield (new ToolCall('tc_l402', new ToolCallData($toolCallId, 'lightning_l402_fetch', $toolArgs), 1100))->withInvocationId('invocation_l402_1');

        yield (new ToolResult('tr_l402', new ToolResultData($toolCallId, 'lightning_l402_fetch', $toolArgs, $toolResult), true, null, 1200))->withInvocationId('invocation_l402_1');

        yield (new TextDelta('td_l402', $messageId, 'done', 1300))->withInvocationId('invocation_l402_1');

        yield (new StreamEnd('end_l402', 'stop', new Usage, 1400))->withInvocationId('invocation_l402_1');
    });

    $orch = resolve(RunOrchestrator::class);

    $resp = $orch->streamAutopilotRun(
        user: $user,
        threadId: $conversationId,
        prompt: 'Hello',
        streamableFactory: fn () => $streamable,
    );

    ob_start();
    $resp->sendContent();
    ob_end_clean();

    $run = DB::table('runs')->where('thread_id', $conversationId)->where('user_id', $user->id)->first();
    expect($run)->not->toBeNull();

    $receipt = DB::table('run_events')->where('run_id', $run->id)->where('type', 'l402_fetch_receipt')->first();
    expect($receipt)->not->toBeNull();

    $payload = json_decode($receipt->payload, true);

    expect($payload['tool_call_id'])->toBe($toolCallId);
    expect($payload['status'])->toBe('completed');
    expect($payload['host'])->toBe('sats4ai.com');
    expect($payload['paid'])->toBe(true);
    expect($payload['quotedAmountMsats'])->toBe(42000);
    expect($payload['proofReference'])->toBe('preimage:deadbeefdeadbeef');
});
