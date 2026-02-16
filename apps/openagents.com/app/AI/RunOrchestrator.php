<?php

namespace App\AI;

use App\AI\Agents\AutopilotAgent;
use Illuminate\Contracts\Auth\Authenticatable;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;
use Laravel\Ai\Streaming\Events\StreamEnd;
use Laravel\Ai\Streaming\Events\StreamStart;
use Laravel\Ai\Streaming\Events\TextDelta;
use Laravel\Ai\Streaming\Events\ToolCall;
use Laravel\Ai\Streaming\Events\ToolResult;
use Symfony\Component\HttpFoundation\StreamedResponse;
use Throwable;

final class RunOrchestrator
{
    /**
     * Stream a single Autopilot "run" for a thread (conversation).
     *
     * Persistence is decoupled from the client connection: if the client disconnects,
     * we continue consuming the model stream and finalize the run in the DB.
     */
    public function streamAutopilotRun(Authenticatable $user, string $threadId, string $prompt, ?callable $streamableFactory = null): StreamedResponse
    {
        $userId = (int) $user->getAuthIdentifier();

        $runId = (string) Str::uuid();
        $now = now();

        $this->ensureThreadExists($threadId, $userId);

        DB::table('runs')->insert([
            'id' => $runId,
            'thread_id' => $threadId,
            'user_id' => $userId,
            'status' => 'running',
            'started_at' => $now,
            'created_at' => $now,
            'updated_at' => $now,
        ]);

        $this->appendEvent($threadId, $runId, $userId, 'run_started', [
            'prompt_sha256' => hash('sha256', $prompt),
            'prompt_chars' => mb_strlen($prompt),
        ]);

        DB::table('messages')->insert([
            'id' => (string) Str::uuid(),
            'thread_id' => $threadId,
            'run_id' => $runId,
            'user_id' => $userId,
            'role' => 'user',
            'content' => $prompt,
            'meta' => null,
            'created_at' => $now,
            'updated_at' => $now,
        ]);

        $streamable = $streamableFactory
            ? $streamableFactory($user, $threadId, $prompt)
            : AutopilotAgent::make()->continue($threadId, $user)->stream($prompt);

        $response = response()->stream(function () use ($streamable, $threadId, $runId, $userId): void {
            $writeToClient = true;
            $shouldFlush = ! app()->runningUnitTests();

            $streamStarted = false;
            $toolCalls = [];
            $toolCallStartedAt = [];
            $toolCallParamsHash = [];
            $lastStreamEndVercel = null;
            /** @var array<string, true> IDs for which we have sent text-start to the client (AI SDK requires text-start before text-delta) */
            $textStartSentForMessageId = [];

            $assistantText = '';
            $modelProvider = null;
            $modelName = null;
            $usage = null;
            $finishReason = null;

            try {
                foreach ($streamable as $event) {
                    // If the client disconnects, stop writing to the socket but continue
                    // consuming the stream to finalize the run.
                    if ($writeToClient && connection_aborted()) {
                        $writeToClient = false;
                    }

                    if ($event instanceof StreamStart) {
                        if ($streamStarted) {
                            continue;
                        }

                        $streamStarted = true;
                        $modelProvider = $event->provider;
                        $modelName = $event->model;

                        DB::table('runs')->where('id', $runId)->update([
                            'model_provider' => $modelProvider,
                            'model' => $modelName,
                            'updated_at' => now(),
                        ]);

                        $this->appendEvent($threadId, $runId, $userId, 'model_stream_started', [
                            'provider' => $modelProvider,
                            'model' => $modelName,
                        ]);
                    }

                    if ($event instanceof ToolCall) {
                        $toolCallId = $event->toolCall->id;

                        $toolCalls[$toolCallId] = true;

                        $paramsCanonical = $this->canonicalJson($event->toolCall->arguments);
                        $paramsHash = hash('sha256', $paramsCanonical);

                        $toolCallStartedAt[$toolCallId] = $event->timestamp;
                        $toolCallParamsHash[$toolCallId] = $paramsHash;

                        $this->appendEvent($threadId, $runId, $userId, 'tool_call_started', [
                            'tool_call_id' => $toolCallId,
                            'tool_name' => $event->toolCall->name,
                            'params_hash' => $paramsHash,
                        ]);
                    }

                    if ($event instanceof ToolResult) {
                        $toolCallId = $event->toolResult->id;

                        if (! isset($toolCalls[$toolCallId])) {
                            continue;
                        }

                        $outputCanonical = $this->canonicalJson($event->toolResult->result);
                        $outputHash = hash('sha256', $outputCanonical);

                        $latencyMs = isset($toolCallStartedAt[$toolCallId])
                            ? max(0, $event->timestamp - $toolCallStartedAt[$toolCallId])
                            : null;

                        $this->appendEvent($threadId, $runId, $userId, $event->successful ? 'tool_call_succeeded' : 'tool_call_failed', [
                            'tool_call_id' => $toolCallId,
                            'tool_name' => $event->toolResult->name,
                            'params_hash' => $toolCallParamsHash[$toolCallId] ?? null,
                            'output_hash' => $outputHash,
                            'latency_ms' => $latencyMs,
                            'error' => $event->successful ? null : $event->error,
                        ]);

                        if (in_array($event->toolResult->name, ['lightning_l402_fetch', 'lightning_l402_approve'], true)) {
                            $r = null;

                            if (is_array($event->toolResult->result)) {
                                $r = $event->toolResult->result;
                            } elseif (is_string($event->toolResult->result)) {
                                $decoded = json_decode($event->toolResult->result, true);
                                if (is_array($decoded)) {
                                    $r = $decoded;
                                }
                            }

                            if (is_array($r)) {
                                $this->appendEvent($threadId, $runId, $userId, 'l402_fetch_receipt', [
                                    'tool_call_id' => $toolCallId,
                                    'tool_name' => is_string($r['toolName'] ?? null) ? $r['toolName'] : 'lightning_l402_fetch',
                                    'status' => $r['status'] ?? null,
                                    'taskId' => $r['taskId'] ?? null,
                                    'host' => $r['host'] ?? null,
                                    'scope' => $r['scope'] ?? null,
                                    'paid' => $r['paid'] ?? null,
                                    'cacheHit' => $r['cacheHit'] ?? null,
                                    'cacheStatus' => $r['cacheStatus'] ?? null,
                                    'approvalRequired' => $r['approvalRequired'] ?? null,
                                    'maxSpendMsats' => $r['maxSpendMsats'] ?? null,
                                    'quotedAmountMsats' => $r['quotedAmountMsats'] ?? null,
                                    'amountMsats' => $r['amountMsats'] ?? null,
                                    'proofReference' => $r['proofReference'] ?? null,
                                    'denyCode' => $r['denyCode'] ?? null,
                                    'responseStatusCode' => $r['responseStatusCode'] ?? null,
                                    'responseBodySha256' => $r['responseBodySha256'] ?? null,
                                ]);
                            }
                        }
                    }

                    if ($event instanceof TextDelta) {
                        $assistantText .= $event->delta;
                    }

                    if ($event instanceof StreamEnd) {
                        $finishReason = $event->reason;
                        $usage = $event->usage->toArray();

                        $this->appendEvent($threadId, $runId, $userId, 'model_finished', [
                            'reason' => $finishReason,
                            'usage' => $usage,
                        ]);

                        // Save until the very end (mirrors laravel/ai Vercel protocol impl).
                        $lastStreamEndVercel = $event->toVercelProtocolArray();

                        continue;
                    }

                    $data = $event->toVercelProtocolArray();
                    if (! is_array($data) || $data === []) {
                        continue;
                    }

                    if ($writeToClient) {
                        // AI SDK requires start-step after start for the stream to be valid.
                        if (($data['type'] ?? '') === 'start') {
                            echo 'data: ' . json_encode($data) . "\n\n";
                            echo 'data: ' . json_encode(['type' => 'start-step']) . "\n\n";
                            if ($shouldFlush) {
                                if (ob_get_level() > 0) {
                                    ob_flush();
                                }
                                flush();
                            }
                            continue;
                        }

                        // AI SDK requires text-start before any text-delta for the same id; inject if missing.
                        if (($data['type'] ?? '') === 'text-delta') {
                            $textId = $data['id'] ?? null;
                            if (is_string($textId) && ! isset($textStartSentForMessageId[$textId])) {
                                echo 'data: ' . json_encode(['type' => 'text-start', 'id' => $textId]) . "\n\n";
                                $textStartSentForMessageId[$textId] = true;
                                if ($shouldFlush) {
                                    if (ob_get_level() > 0) {
                                        ob_flush();
                                    }
                                    flush();
                                }
                            }
                        }
                        if (($data['type'] ?? '') === 'text-start') {
                            $textId = $data['id'] ?? null;
                            if (is_string($textId)) {
                                $textStartSentForMessageId[$textId] = true;
                            }
                        }

                        echo 'data: ' . json_encode($data) . "\n\n";

                        if ($shouldFlush) {
                            if (ob_get_level() > 0) {
                                ob_flush();
                            }
                            flush();
                        }
                    }
                }

                $now = now();

                DB::table('messages')->insert([
                    'id' => (string) Str::uuid(),
                    'thread_id' => $threadId,
                    'run_id' => $runId,
                    'user_id' => $userId,
                    'role' => 'assistant',
                    'content' => $assistantText,
                    'meta' => null,
                    'created_at' => $now,
                    'updated_at' => $now,
                ]);

                DB::table('runs')->where('id', $runId)->update([
                    'status' => 'completed',
                    'usage' => is_array($usage) ? json_encode($usage) : null,
                    'meta' => ($modelProvider || $modelName) ? json_encode(['provider' => $modelProvider, 'model' => $modelName]) : null,
                    'completed_at' => $now,
                    'updated_at' => $now,
                ]);

                $this->appendEvent($threadId, $runId, $userId, 'run_completed', [
                    'assistant_sha256' => hash('sha256', $assistantText),
                    'assistant_chars' => mb_strlen($assistantText),
                ]);

                if ($writeToClient) {
                    // AI SDK expects finish-step before finish.
                    echo 'data: ' . json_encode(['type' => 'finish-step']) . "\n\n";
                    if (is_array($lastStreamEndVercel) && $lastStreamEndVercel !== []) {
                        echo 'data: ' . json_encode($lastStreamEndVercel) . "\n\n";
                    }

                    echo "data: [DONE]\n\n";

                    if ($shouldFlush) {
                        if (ob_get_level() > 0) {
                            ob_flush();
                        }
                        flush();
                    }
                }
            } catch (Throwable $e) {
                $now = now();

                DB::table('runs')->where('id', $runId)->update([
                    'status' => 'failed',
                    'error' => $e->getMessage(),
                    'completed_at' => $now,
                    'updated_at' => $now,
                ]);

                $this->appendEvent($threadId, $runId, $userId, 'run_failed', [
                    'error' => $e->getMessage(),
                ]);

                if ($writeToClient) {
                    // Best-effort graceful shutdown of the SSE stream.
                    echo 'data: ' . json_encode(['type' => 'finish-step']) . "\n\n";
                    echo "data: [DONE]\n\n";

                    if ($shouldFlush) {
                        if (ob_get_level() > 0) {
                            ob_flush();
                        }
                        flush();
                    }
                }
            }
        }, 200, [
            'Cache-Control' => 'no-cache, no-transform',
            'Content-Type' => 'text/event-stream',
            'x-vercel-ai-ui-message-stream' => 'v1',
            'x-oa-run-id' => $runId,
            'x-accel-buffering' => 'no',
        ]);

        return tap($response, function (StreamedResponse $r) use ($runId): void {
            $r->headers->set('x-oa-run-id', $runId);
        });
    }

    private function canonicalJson(mixed $value): string
    {
        return json_encode($this->canonicalize($value), JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
    }

    private function canonicalize(mixed $value): mixed
    {
        if (is_array($value)) {
            if (array_is_list($value)) {
                return array_map(fn($v) => $this->canonicalize($v), $value);
            }

            ksort($value);

            $out = [];
            foreach ($value as $k => $v) {
                $out[$k] = $this->canonicalize($v);
            }

            return $out;
        }

        if (is_object($value)) {
            return $this->canonicalize((array) $value);
        }

        return $value;
    }

    private function ensureThreadExists(string $threadId, int $userId): void
    {
        $exists = DB::table('threads')->where('id', $threadId)->where('user_id', $userId)->exists();
        if ($exists) {
            return;
        }

        $title = DB::table('agent_conversations')->where('id', $threadId)->where('user_id', $userId)->value('title');
        if (! is_string($title) || $title === '') {
            $title = 'New conversation';
        }

        $now = now();

        DB::table('threads')->insert([
            'id' => $threadId,
            'user_id' => $userId,
            'title' => $title,
            'created_at' => $now,
            'updated_at' => $now,
        ]);
    }

    private function appendEvent(string $threadId, string $runId, int $userId, string $type, ?array $payload = null): void
    {
        DB::table('run_events')->insert([
            'thread_id' => $threadId,
            'run_id' => $runId,
            'user_id' => $userId,
            'type' => $type,
            'payload' => is_array($payload) ? json_encode($payload) : null,
            'created_at' => now(),
        ]);
    }
}
