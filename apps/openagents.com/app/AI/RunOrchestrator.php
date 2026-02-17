<?php

namespace App\AI;

use App\AI\Agents\AutopilotAgent;
use App\AI\Agents\ErrorExplainerAgent;
use App\AI\Runtime\AutopilotExecutionContext;
use App\AI\Tools\AutopilotToolResolver;
use App\Services\PostHogService;
use Illuminate\Contracts\Auth\Authenticatable;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Log;
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
    public function streamAutopilotRun(
        Authenticatable $user,
        string $threadId,
        string $prompt,
        bool $authenticatedSession = true,
        ?callable $streamableFactory = null,
    ): StreamedResponse {
        $userId = (int) $user->getAuthIdentifier();
        $userEmail = $user->email ?? 'unknown';

        $runId = (string) Str::uuid();
        $now = now();

        $threadContext = $this->resolveThreadContext($threadId, $userId);
        $autopilotId = $threadContext['autopilotId'];
        $autopilotConfigVersion = $threadContext['autopilotConfigVersion'];

        DB::table('runs')->insert([
            'id' => $runId,
            'thread_id' => $threadId,
            'user_id' => $userId,
            'autopilot_id' => $autopilotId,
            'autopilot_config_version' => $autopilotConfigVersion,
            'status' => 'running',
            'started_at' => $now,
            'created_at' => $now,
            'updated_at' => $now,
        ]);

        $this->appendEvent(
            threadId: $threadId,
            runId: $runId,
            userId: $userId,
            type: 'run_started',
            payload: [
                'prompt_sha256' => hash('sha256', $prompt),
                'prompt_chars' => mb_strlen($prompt),
            ],
            autopilotId: $autopilotId,
            actorType: 'user',
        );

        DB::table('messages')->insert([
            'id' => (string) Str::uuid(),
            'thread_id' => $threadId,
            'run_id' => $runId,
            'user_id' => $userId,
            'autopilot_id' => $autopilotId,
            'role' => 'user',
            'content' => $prompt,
            'meta' => null,
            'created_at' => $now,
            'updated_at' => $now,
        ]);

        $executionContext = resolve(AutopilotExecutionContext::class);
        $executionContext->set($userId, $autopilotId, $authenticatedSession);

        $runtimeActorType = $this->runtimeActorType($autopilotId);
        $runtimeActorAutopilotId = $runtimeActorType === 'autopilot' ? $autopilotId : null;

        $toolResolution = resolve(AutopilotToolResolver::class)->resolutionForAutopilot($autopilotId);

        $this->appendEvent(
            threadId: $threadId,
            runId: $runId,
            userId: $userId,
            type: 'tool_policy_applied',
            payload: $toolResolution['audit'] ?? [],
            autopilotId: $autopilotId,
            actorType: $runtimeActorType,
            actorAutopilotId: $runtimeActorAutopilotId,
        );

        try {
            $streamable = $streamableFactory
                ? $streamableFactory($user, $threadId, $prompt)
                : AutopilotAgent::make()->continue($threadId, $user)->stream($prompt);
        } catch (Throwable $e) {
            $executionContext->clear();

            return $this->streamErrorOnlyResponse($e, $runId, $threadId, $userId, $userEmail, $autopilotId);
        }

        $response = response()->stream(function () use ($streamable, $threadId, $runId, $userId, $userEmail, $autopilotId, $executionContext): void {
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
            $runtimeActorType = $this->runtimeActorType($autopilotId);
            $runtimeActorAutopilotId = $runtimeActorType === 'autopilot' ? $autopilotId : null;

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

                        $this->appendEvent(
                            threadId: $threadId,
                            runId: $runId,
                            userId: $userId,
                            type: 'model_stream_started',
                            payload: [
                                'provider' => $modelProvider,
                                'model' => $modelName,
                            ],
                            autopilotId: $autopilotId,
                            actorType: $runtimeActorType,
                            actorAutopilotId: $runtimeActorAutopilotId,
                        );
                    }

                    if ($event instanceof ToolCall) {
                        $toolCallId = $event->toolCall->id;

                        $toolCalls[$toolCallId] = true;

                        $paramsCanonical = $this->canonicalJson($event->toolCall->arguments);
                        $paramsHash = hash('sha256', $paramsCanonical);

                        $toolCallStartedAt[$toolCallId] = $event->timestamp;
                        $toolCallParamsHash[$toolCallId] = $paramsHash;

                        $this->appendEvent(
                            threadId: $threadId,
                            runId: $runId,
                            userId: $userId,
                            type: 'tool_call_started',
                            payload: [
                                'tool_call_id' => $toolCallId,
                                'tool_name' => $event->toolCall->name,
                                'params_hash' => $paramsHash,
                            ],
                            autopilotId: $autopilotId,
                            actorType: $runtimeActorType,
                            actorAutopilotId: $runtimeActorAutopilotId,
                        );
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

                        $this->appendEvent(
                            threadId: $threadId,
                            runId: $runId,
                            userId: $userId,
                            type: $event->successful ? 'tool_call_succeeded' : 'tool_call_failed',
                            payload: [
                                'tool_call_id' => $toolCallId,
                                'tool_name' => $event->toolResult->name,
                                'params_hash' => $toolCallParamsHash[$toolCallId] ?? null,
                                'output_hash' => $outputHash,
                                'latency_ms' => $latencyMs,
                                'error' => $event->successful ? null : $event->error,
                            ],
                            autopilotId: $autopilotId,
                            actorType: $runtimeActorType,
                            actorAutopilotId: $runtimeActorAutopilotId,
                        );

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
                                $this->appendEvent(
                                    threadId: $threadId,
                                    runId: $runId,
                                    userId: $userId,
                                    type: 'l402_fetch_receipt',
                                    payload: [
                                        'tool_call_id' => $toolCallId,
                                        'tool_name' => is_string($r['toolName'] ?? null) ? $r['toolName'] : 'lightning_l402_fetch',
                                        'status' => $r['status'] ?? null,
                                        'taskId' => $r['taskId'] ?? null,
                                        'host' => $r['host'] ?? null,
                                        'scope' => $r['scope'] ?? null,
                                        'paid' => $r['paid'] ?? null,
                                        'cacheHit' => $r['cacheHit'] ?? null,
                                        'cacheStatus' => $r['cacheStatus'] ?? null,
                                        'approvalRequired' => $r['requireApproval'] ?? $r['approvalRequired'] ?? null,
                                        'maxSpendMsats' => $r['maxSpendMsats'] ?? null,
                                        'quotedAmountMsats' => $r['quotedAmountMsats'] ?? null,
                                        'amountMsats' => $r['amountMsats'] ?? null,
                                        'proofReference' => $r['proofReference'] ?? null,
                                        'denyCode' => $r['denyCode'] ?? null,
                                        'responseStatusCode' => $r['responseStatusCode'] ?? null,
                                        'responseBodySha256' => $r['responseBodySha256'] ?? null,
                                    ],
                                    autopilotId: $autopilotId,
                                    actorType: $runtimeActorType,
                                    actorAutopilotId: $runtimeActorAutopilotId,
                                );

                                // PostHog: Track L402 payment if paid
                                if (($r['paid'] ?? false) === true && isset($r['amountMsats'])) {
                                    $posthog = resolve(PostHogService::class);
                                    $posthog->capture($userEmail, 'l402 payment made', [
                                        'run_id' => $runId,
                                        'thread_id' => $threadId,
                                        'host' => $r['host'] ?? null,
                                        'amount_msats' => $r['amountMsats'] ?? null,
                                    ]);
                                }
                            }
                        }
                    }

                    if ($event instanceof TextDelta) {
                        $assistantText .= $event->delta;
                    }

                    if ($event instanceof StreamEnd) {
                        $finishReason = $event->reason;
                        $usage = $event->usage->toArray();

                        $this->appendEvent(
                            threadId: $threadId,
                            runId: $runId,
                            userId: $userId,
                            type: 'model_finished',
                            payload: [
                                'reason' => $finishReason,
                                'usage' => $usage,
                            ],
                            autopilotId: $autopilotId,
                            actorType: $runtimeActorType,
                            actorAutopilotId: $runtimeActorAutopilotId,
                        );

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
                            echo 'data: '.json_encode($data)."\n\n";
                            echo 'data: '.json_encode(['type' => 'start-step'])."\n\n";
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
                                echo 'data: '.json_encode(['type' => 'text-start', 'id' => $textId])."\n\n";
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

                        echo 'data: '.json_encode($data)."\n\n";

                        if ($shouldFlush) {
                            if (ob_get_level() > 0) {
                                ob_flush();
                            }
                            flush();
                        }
                    }
                }

                $now = now();

                if (trim($assistantText) === '') {
                    $assistantText = "I couldn't generate a response from the model. Please try again.";

                    $this->appendEvent(
                        threadId: $threadId,
                        runId: $runId,
                        userId: $userId,
                        type: 'model_empty_response',
                        payload: [
                            'reason' => $finishReason,
                            'usage' => $usage,
                        ],
                        autopilotId: $autopilotId,
                        actorType: $runtimeActorType,
                        actorAutopilotId: $runtimeActorAutopilotId,
                    );

                    if ($writeToClient) {
                        $this->emitSyntheticAssistantText($assistantText, $runId, $streamStarted, $shouldFlush);
                    }
                }

                DB::table('messages')->insert([
                    'id' => (string) Str::uuid(),
                    'thread_id' => $threadId,
                    'run_id' => $runId,
                    'user_id' => $userId,
                    'autopilot_id' => $autopilotId,
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

                $this->appendEvent(
                    threadId: $threadId,
                    runId: $runId,
                    userId: $userId,
                    type: 'run_completed',
                    payload: [
                        'assistant_sha256' => hash('sha256', $assistantText),
                        'assistant_chars' => mb_strlen($assistantText),
                    ],
                    autopilotId: $autopilotId,
                    actorType: $runtimeActorType,
                    actorAutopilotId: $runtimeActorAutopilotId,
                );

                // PostHog: Track chat run completed
                $posthog = resolve(PostHogService::class);
                $posthog->capture($userEmail, 'chat run completed', [
                    'run_id' => $runId,
                    'thread_id' => $threadId,
                    'model_provider' => $modelProvider,
                    'model' => $modelName,
                    'input_tokens' => $usage['inputTokens'] ?? null,
                    'output_tokens' => $usage['outputTokens'] ?? null,
                ]);

                if ($writeToClient) {
                    // AI SDK expects finish-step before finish.
                    echo 'data: '.json_encode(['type' => 'finish-step'])."\n\n";
                    if (is_array($lastStreamEndVercel) && $lastStreamEndVercel !== []) {
                        echo 'data: '.json_encode($lastStreamEndVercel)."\n\n";
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

                Log::error('Chat run failed', [
                    'run_id' => $runId,
                    'thread_id' => $threadId,
                    'error' => $e->getMessage(),
                    'exception' => get_class($e),
                    'trace' => $e->getTraceAsString(),
                ]);

                DB::table('runs')->where('id', $runId)->update([
                    'status' => 'failed',
                    'error' => $e->getMessage(),
                    'completed_at' => $now,
                    'updated_at' => $now,
                ]);

                $assistantText = $this->streamErrorExplanationFromLlm(
                    $e,
                    $runId,
                    $streamStarted,
                    $writeToClient,
                    $shouldFlush,
                );
                // If explainer threw or returned empty, streamStarted may still be false; emit fallback below if needed.

                DB::table('messages')->insert([
                    'id' => (string) Str::uuid(),
                    'thread_id' => $threadId,
                    'run_id' => $runId,
                    'user_id' => $userId,
                    'autopilot_id' => $autopilotId,
                    'role' => 'assistant',
                    'content' => $assistantText,
                    'meta' => json_encode([
                        'error' => 'run_failed',
                    ]),
                    'created_at' => $now,
                    'updated_at' => $now,
                ]);

                $this->appendEvent(
                    threadId: $threadId,
                    runId: $runId,
                    userId: $userId,
                    type: 'run_failed',
                    payload: [
                        'error' => $e->getMessage(),
                    ],
                    autopilotId: $autopilotId,
                    actorType: 'system',
                );

                // PostHog: Track chat run failed
                $posthog = resolve(PostHogService::class);
                $posthog->capture($userEmail, 'chat run failed', [
                    'run_id' => $runId,
                    'thread_id' => $threadId,
                    'error' => $e->getMessage(),
                ]);

                if ($writeToClient) {
                    // If we didn't stream (explainer failed or returned nothing), emit the message we have.
                    if (trim($assistantText) !== '' && ! $streamStarted) {
                        $this->emitSyntheticAssistantText($assistantText, $runId, $streamStarted, $shouldFlush);
                    }
                    // Best-effort graceful shutdown of the SSE stream.
                    echo 'data: '.json_encode(['type' => 'finish-step'])."\n\n";
                    echo "data: [DONE]\n\n";

                    if ($shouldFlush) {
                        if (ob_get_level() > 0) {
                            ob_flush();
                        }
                        flush();
                    }
                }
            } finally {
                $executionContext->clear();
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

    /**
     * When stream creation fails (e.g. initial API error), return an SSE stream that
     * runs only the error path: LLM explains the error, then finish.
     */
    private function streamErrorOnlyResponse(Throwable $e, string $runId, string $threadId, int $userId, string $userEmail, ?string $autopilotId): StreamedResponse
    {
        Log::error('Chat run failed (stream creation)', [
            'run_id' => $runId,
            'thread_id' => $threadId,
            'error' => $e->getMessage(),
            'exception' => get_class($e),
            'trace' => $e->getTraceAsString(),
        ]);

        $now = now();
        DB::table('runs')->where('id', $runId)->update([
            'status' => 'failed',
            'error' => $e->getMessage(),
            'completed_at' => $now,
            'updated_at' => $now,
        ]);

        $shouldFlush = ! app()->runningUnitTests();

        $response = response()->stream(function () use ($e, $runId, $threadId, $userId, $userEmail, $autopilotId, $shouldFlush): void {
            $streamStarted = false;
            $assistantText = $this->streamErrorExplanationFromLlm($e, $runId, $streamStarted, true, $shouldFlush);

            $now = now();
            DB::table('messages')->insert([
                'id' => (string) Str::uuid(),
                'thread_id' => $threadId,
                'run_id' => $runId,
                'user_id' => $userId,
                'autopilot_id' => $autopilotId,
                'role' => 'assistant',
                'content' => $assistantText,
                'meta' => json_encode(['error' => 'run_failed']),
                'created_at' => $now,
                'updated_at' => $now,
            ]);

            $this->appendEvent(
                threadId: $threadId,
                runId: $runId,
                userId: $userId,
                type: 'run_failed',
                payload: ['error' => $e->getMessage()],
                autopilotId: $autopilotId,
                actorType: 'system',
            );

            $posthog = resolve(PostHogService::class);
            $posthog->capture($userEmail, 'chat run failed', [
                'run_id' => $runId,
                'thread_id' => $threadId,
                'error' => $e->getMessage(),
            ]);

            if (trim($assistantText) !== '' && ! $streamStarted) {
                $this->emitSyntheticAssistantText($assistantText, $runId, $streamStarted, $shouldFlush);
            }
            echo 'data: '.json_encode(['type' => 'finish-step'])."\n\n";
            echo "data: [DONE]\n\n";
            if ($shouldFlush) {
                if (ob_get_level() > 0) {
                    ob_flush();
                }
                flush();
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

    /**
     * Ask the LLM to explain the error to the user; stream to client when possible.
     * Returns the assistant text (from LLM or fallback). Sets $streamStarted when streaming.
     */
    private function streamErrorExplanationFromLlm(Throwable $e, string $runId, bool &$streamStarted, bool $writeToClient, bool $shouldFlush): string
    {
        $fallback = 'I ran into an internal error while generating a response. Please retry.';
        $prompt = 'The system encountered this error while generating a response: '.$e->getMessage()."\n\nExplain to the user what went wrong in simple, friendly terms. Be brief.";

        try {
            $streamable = ErrorExplainerAgent::make()->stream($prompt);
        } catch (Throwable) {
            return $fallback;
        }

        $assistantText = '';
        $messageId = 'error-explainer-'.$runId;
        $textStartEmitted = false;

        try {
            foreach ($streamable as $event) {
                if ($event instanceof TextDelta) {
                    if ($writeToClient && ! $streamStarted) {
                        echo 'data: '.json_encode(['type' => 'start'])."\n\n";
                        echo 'data: '.json_encode(['type' => 'start-step'])."\n\n";
                        $this->flushStream($shouldFlush);
                        $streamStarted = true;
                    }
                    if ($writeToClient && ! $textStartEmitted) {
                        echo 'data: '.json_encode(['type' => 'text-start', 'id' => $messageId])."\n\n";
                        $this->flushStream($shouldFlush);
                        $textStartEmitted = true;
                    }
                    $assistantText .= $event->delta;
                    if ($writeToClient) {
                        echo 'data: '.json_encode(['type' => 'text-delta', 'id' => $messageId, 'delta' => $event->delta])."\n\n";
                        $this->flushStream($shouldFlush);
                    }
                }
            }
        } catch (Throwable) {
            return trim($assistantText) !== '' ? $assistantText : $fallback;
        }

        if ($writeToClient && $textStartEmitted) {
            echo 'data: '.json_encode(['type' => 'text-end', 'id' => $messageId])."\n\n";
            $this->flushStream($shouldFlush);
        }

        return trim($assistantText) !== '' ? $assistantText : $fallback;
    }

    private function emitSyntheticAssistantText(string $text, string $runId, bool &$streamStarted, bool $shouldFlush): void
    {
        if ($text === '') {
            return;
        }

        if (! $streamStarted) {
            echo 'data: '.json_encode(['type' => 'start'])."\n\n";
            echo 'data: '.json_encode(['type' => 'start-step'])."\n\n";
            $this->flushStream($shouldFlush);
            $streamStarted = true;
        }

        $messageId = 'fallback-'.$runId;
        echo 'data: '.json_encode(['type' => 'text-start', 'id' => $messageId])."\n\n";
        echo 'data: '.json_encode(['type' => 'text-delta', 'id' => $messageId, 'delta' => $text])."\n\n";
        echo 'data: '.json_encode(['type' => 'text-end', 'id' => $messageId])."\n\n";
        $this->flushStream($shouldFlush);
    }

    private function flushStream(bool $shouldFlush): void
    {
        if (! $shouldFlush) {
            return;
        }

        if (ob_get_level() > 0) {
            ob_flush();
        }

        flush();
    }

    private function canonicalJson(mixed $value): string
    {
        return json_encode($this->canonicalize($value), JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
    }

    private function canonicalize(mixed $value): mixed
    {
        if (is_array($value)) {
            if (array_is_list($value)) {
                return array_map(fn ($v) => $this->canonicalize($v), $value);
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

    /**
     * @return array{autopilotId: ?string, autopilotConfigVersion: ?int}
     */
    private function resolveThreadContext(string $threadId, int $userId): array
    {
        $thread = DB::table('threads')
            ->where('id', $threadId)
            ->where('user_id', $userId)
            ->first(['id', 'autopilot_id']);

        if (! $thread) {
            $title = DB::table('agent_conversations')->where('id', $threadId)->where('user_id', $userId)->value('title');
            if (! is_string($title) || trim($title) === '') {
                $title = 'New conversation';
            }

            $now = now();

            DB::table('threads')->insert([
                'id' => $threadId,
                'user_id' => $userId,
                'autopilot_id' => null,
                'title' => $title,
                'created_at' => $now,
                'updated_at' => $now,
            ]);

            return [
                'autopilotId' => null,
                'autopilotConfigVersion' => null,
            ];
        }

        $autopilotId = is_string($thread->autopilot_id ?? null) && trim((string) $thread->autopilot_id) !== ''
            ? trim((string) $thread->autopilot_id)
            : null;

        if ($autopilotId === null) {
            return [
                'autopilotId' => null,
                'autopilotConfigVersion' => null,
            ];
        }

        $configVersion = DB::table('autopilots')->where('id', $autopilotId)->value('config_version');

        return [
            'autopilotId' => $autopilotId,
            'autopilotConfigVersion' => is_numeric($configVersion) ? (int) $configVersion : null,
        ];
    }

    private function runtimeActorType(?string $autopilotId): string
    {
        return is_string($autopilotId) && $autopilotId !== '' ? 'autopilot' : 'system';
    }

    private function appendEvent(
        string $threadId,
        string $runId,
        int $userId,
        string $type,
        ?array $payload = null,
        ?string $autopilotId = null,
        string $actorType = 'user',
        ?string $actorAutopilotId = null,
    ): void {
        $resolvedActorType = in_array($actorType, ['user', 'autopilot', 'system'], true)
            ? $actorType
            : 'system';

        if ($resolvedActorType !== 'autopilot') {
            $actorAutopilotId = null;
        } elseif (! is_string($actorAutopilotId) || trim($actorAutopilotId) === '') {
            $actorAutopilotId = $autopilotId;
        }

        DB::table('run_events')->insert([
            'thread_id' => $threadId,
            'run_id' => $runId,
            'user_id' => $userId,
            'autopilot_id' => $autopilotId,
            'actor_type' => $resolvedActorType,
            'actor_autopilot_id' => $actorAutopilotId,
            'type' => $type,
            'payload' => is_array($payload) ? json_encode($payload) : null,
            'created_at' => now(),
        ]);
    }
}
