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

function canonical_json_for_hash(mixed $value): string
{
    $canonicalize = function ($v) use (&$canonicalize) {
        if (is_array($v)) {
            $isList = array_is_list($v);
            if ($isList) {
                return array_map(fn ($x) => $canonicalize($x), $v);
            }
            ksort($v);
            $out = [];
            foreach ($v as $k => $x) {
                $out[$k] = $canonicalize($x);
            }

            return $out;
        }

        if (is_object($v)) {
            return $canonicalize((array) $v);
        }

        return $v;
    };

    return json_encode($canonicalize($value), JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
}

test('tool call events are persisted with deterministic hashes', function () {
    $user = User::factory()->create();

    $conversationId = (string) Str::uuid7();

    DB::table('agent_conversations')->insert([
        'id' => $conversationId,
        'user_id' => $user->id,
        'title' => 'Tool test',
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    $toolCallId = 'toolcall_1';
    $messageId = 'msg_1';

    $toolArgs = ['text' => 'hello'];
    $toolResult = 'hello';

    $expectedParamsHash = hash('sha256', canonical_json_for_hash($toolArgs));
    $expectedOutputHash = hash('sha256', canonical_json_for_hash($toolResult));

    $streamable = new StreamableAgentResponse('invocation_1', function () use ($messageId, $toolCallId, $toolArgs, $toolResult) {
        yield (new StreamStart('start_1', 'fake', 'fake-model', 1000))->withInvocationId('invocation_1');

        yield (new ToolCall('tc_1', new ToolCallData($toolCallId, 'echo', $toolArgs), 1100))->withInvocationId('invocation_1');

        yield (new ToolResult('tr_1', new ToolResultData($toolCallId, 'echo', $toolArgs, $toolResult), true, null, 1200))->withInvocationId('invocation_1');

        yield (new TextDelta('td_1', $messageId, 'done', 1300))->withInvocationId('invocation_1');

        yield (new StreamEnd('end_1', 'stop', new Usage, 1400))->withInvocationId('invocation_1');
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
    $body = (string) ob_get_clean();

    expect($body)->toContain('tool-input-available');
    expect($body)->toContain('tool-output-available');

    $run = DB::table('runs')->where('thread_id', $conversationId)->where('user_id', $user->id)->first();
    expect($run)->not->toBeNull();

    $toolStart = DB::table('run_events')->where('run_id', $run->id)->where('type', 'tool_call_started')->first();
    expect($toolStart)->not->toBeNull();

    $toolStartPayload = json_decode($toolStart->payload, true);
    expect($toolStartPayload['tool_name'])->toBe('echo');
    expect($toolStartPayload['tool_call_id'])->toBe($toolCallId);
    expect($toolStartPayload['params_hash'])->toBe($expectedParamsHash);

    $toolDone = DB::table('run_events')->where('run_id', $run->id)->where('type', 'tool_call_succeeded')->first();
    expect($toolDone)->not->toBeNull();

    $toolDonePayload = json_decode($toolDone->payload, true);
    expect($toolDonePayload['tool_name'])->toBe('echo');
    expect($toolDonePayload['tool_call_id'])->toBe($toolCallId);
    expect($toolDonePayload['params_hash'])->toBe($expectedParamsHash);
    expect($toolDonePayload['output_hash'])->toBe($expectedOutputHash);
    expect($toolDonePayload['latency_ms'])->toBe(100);
});
