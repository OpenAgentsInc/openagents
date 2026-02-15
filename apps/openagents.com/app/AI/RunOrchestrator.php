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
    public function streamAutopilotRun(Authenticatable $user, string $threadId, string $prompt): StreamedResponse
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

        $agent = AutopilotAgent::make()->continue($threadId, $user);
        $streamable = $agent->stream($prompt);

        $response = response()->stream(function () use ($streamable, $threadId, $runId, $userId): void {
            $writeToClient = true;

            $streamStarted = false;
            $toolCalls = [];
            $lastStreamEndVercel = null;

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
                        $toolCalls[$event->toolCall->id] = true;
                    }

                    if ($event instanceof ToolResult && ! isset($toolCalls[$event->toolResult->id])) {
                        continue;
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
                        echo 'data: '.json_encode($data)."\n\n";

                        if (ob_get_level() > 0) {
                            ob_flush();
                        }
                        flush();
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
                    if (is_array($lastStreamEndVercel) && $lastStreamEndVercel !== []) {
                        echo 'data: '.json_encode($lastStreamEndVercel)."\n\n";
                    }

                    echo "data: [DONE]\n\n";

                    if (ob_get_level() > 0) {
                        ob_flush();
                    }
                    flush();
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
                    echo "data: [DONE]\n\n";

                    if (ob_get_level() > 0) {
                        ob_flush();
                    }
                    flush();
                }
            }
        }, 200, [
            'Cache-Control' => 'no-cache, no-transform',
            'Content-Type' => 'text/event-stream',
            'x-vercel-ai-ui-message-stream' => 'v1',
            'x-oa-run-id' => $runId,
        ]);

        return tap($response, function (StreamedResponse $r) use ($runId): void {
            $r->headers->set('x-oa-run-id', $runId);
        });
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
