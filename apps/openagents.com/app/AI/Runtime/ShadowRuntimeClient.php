<?php

namespace App\AI\Runtime;

use Illuminate\Contracts\Auth\Authenticatable;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Str;
use Symfony\Component\HttpFoundation\StreamedResponse;
use Throwable;

final class ShadowRuntimeClient implements RuntimeClient
{
    public function __construct(
        private readonly RuntimeClient $primaryClient,
        private readonly RuntimeClient $shadowClient,
    ) {}

    public function driverName(): string
    {
        return $this->primaryClient->driverName();
    }

    public function streamAutopilotRun(
        Authenticatable $user,
        string $threadId,
        string $prompt,
        bool $authenticatedSession = true,
    ): StreamedResponse {
        $primaryResponse = $this->primaryClient->streamAutopilotRun(
            user: $user,
            threadId: $threadId,
            prompt: $prompt,
            authenticatedSession: $authenticatedSession,
        );

        if (! $this->shouldMirror($user, $threadId)) {
            return $primaryResponse;
        }

        $primaryCallback = $primaryResponse->getCallback();
        if (! is_callable($primaryCallback)) {
            return $primaryResponse;
        }

        return response()->stream(function () use (
            $primaryCallback,
            $user,
            $threadId,
            $prompt,
            $authenticatedSession
        ): void {
            $maxCaptureBytes = max(2_000, (int) config('runtime.shadow.max_capture_bytes', 200_000));

            $primaryStream = $this->captureCallbackOutput($primaryCallback, true, $maxCaptureBytes);
            $primarySummary = $this->summarizeSseStream($primaryStream);

            $shadowStream = null;
            $shadowSummary = null;
            $diff = ['pass' => false, 'mismatches' => ['shadow_unavailable']];
            $status = 'shadow_failed';
            $shadowError = null;

            try {
                $shadowResponse = $this->shadowClient->streamAutopilotRun(
                    user: $user,
                    threadId: $threadId,
                    prompt: $prompt,
                    authenticatedSession: $authenticatedSession,
                );

                $shadowCallback = $shadowResponse->getCallback();

                if (is_callable($shadowCallback)) {
                    $shadowStream = $this->captureCallbackOutput($shadowCallback, false, $maxCaptureBytes);
                    $shadowSummary = $this->summarizeSseStream($shadowStream);
                    $diff = $this->compareSummaries($primarySummary, $shadowSummary);
                    $status = 'compared';
                } else {
                    $status = 'shadow_missing_callback';
                    $diff = ['pass' => false, 'mismatches' => ['shadow_missing_callback']];
                }
            } catch (Throwable $e) {
                $shadowError = $e->getMessage();
                Log::warning('Shadow runtime mirror failed', ['error' => $shadowError]);
            }

            $this->persistShadowDiff(
                userId: (int) $user->getAuthIdentifier(),
                threadId: $threadId,
                requestMeta: [
                    'prompt_sha256' => hash('sha256', $prompt),
                    'prompt_chars' => mb_strlen($prompt),
                    'authenticated_session' => $authenticatedSession,
                ],
                primarySummary: $primarySummary,
                shadowSummary: $shadowSummary,
                diff: $diff + ['shadow_error' => $shadowError],
                status: $status,
            );
        }, $primaryResponse->getStatusCode(), $primaryResponse->headers->allPreserveCaseWithoutCookies());
    }

    private function shouldMirror(Authenticatable $user, string $threadId): bool
    {
        $sampleRate = (float) config('runtime.shadow.sample_rate', 1.0);

        if ($sampleRate <= 0.0) {
            return false;
        }

        if ($sampleRate >= 1.0) {
            return true;
        }

        $key = $threadId.'|'.(string) $user->getAuthIdentifier();
        $bucket = hexdec(substr(hash('sha256', $key), 0, 8)) % 10_000;

        return $bucket < (int) round($sampleRate * 10_000);
    }

    private function captureCallbackOutput(callable $callback, bool $forwardToClient, int $maxCaptureBytes): string
    {
        $captured = '';

        ob_start(function (string $chunk) use (&$captured, $forwardToClient, $maxCaptureBytes): string {
            if (strlen($captured) < $maxCaptureBytes) {
                $remaining = $maxCaptureBytes - strlen($captured);
                $captured .= substr($chunk, 0, $remaining);
            }

            return $forwardToClient ? $chunk : '';
        }, 1);

        try {
            $callback();
        } finally {
            if (ob_get_level() > 0) {
                if ($forwardToClient) {
                    ob_end_flush();
                } else {
                    ob_end_clean();
                }
            }
        }

        return $captured;
    }

    /**
     * @return array<string, mixed>
     */
    private function summarizeSseStream(string $stream): array
    {
        $chunks = preg_split("/\n\n+/", $stream) ?: [];
        $types = [];
        $textDeltaCount = 0;
        $toolCallCount = 0;
        $hasDone = false;

        foreach ($chunks as $chunk) {
            $lines = preg_split("/\r?\n/", $chunk) ?: [];

            foreach ($lines as $line) {
                $line = trim($line);
                if (! str_starts_with($line, 'data: ')) {
                    continue;
                }

                $payload = trim(substr($line, 6));

                if ($payload === '[DONE]') {
                    $hasDone = true;

                    continue;
                }

                $decoded = json_decode($payload, true);
                if (! is_array($decoded)) {
                    continue;
                }

                $type = (string) ($decoded['type'] ?? '');
                if ($type === '') {
                    continue;
                }

                $types[] = $type;
                if ($type === 'text-delta') {
                    $textDeltaCount++;
                }
                if ($type === 'tool-call') {
                    $toolCallCount++;
                }
            }
        }

        return [
            'event_types' => $types,
            'event_type_counts' => array_count_values($types),
            'text_delta_count' => $textDeltaCount,
            'tool_call_count' => $toolCallCount,
            'has_start' => in_array('start', $types, true),
            'has_finish' => in_array('finish', $types, true),
            'has_done' => $hasDone,
            'terminal_type' => $types === [] ? null : $types[array_key_last($types)],
            'stream_sha256' => hash('sha256', $stream),
        ];
    }

    /**
     * @param  array<string, mixed>  $primary
     * @param  array<string, mixed>  $shadow
     * @return array<string, mixed>
     */
    private function compareSummaries(array $primary, array $shadow): array
    {
        $mismatches = [];

        foreach (['has_start', 'has_finish', 'has_done'] as $flag) {
            if (($primary[$flag] ?? null) !== ($shadow[$flag] ?? null)) {
                $mismatches[] = $flag;
            }
        }

        if ((int) ($primary['text_delta_count'] ?? 0) > 0 !== ((int) ($shadow['text_delta_count'] ?? 0) > 0)) {
            $mismatches[] = 'text_delta_presence';
        }

        if (($primary['terminal_type'] ?? null) !== ($shadow['terminal_type'] ?? null)) {
            $mismatches[] = 'terminal_type';
        }

        return [
            'pass' => $mismatches === [],
            'mismatches' => $mismatches,
            'primary' => [
                'text_delta_count' => (int) ($primary['text_delta_count'] ?? 0),
                'tool_call_count' => (int) ($primary['tool_call_count'] ?? 0),
                'terminal_type' => $primary['terminal_type'] ?? null,
            ],
            'shadow' => [
                'text_delta_count' => (int) ($shadow['text_delta_count'] ?? 0),
                'tool_call_count' => (int) ($shadow['tool_call_count'] ?? 0),
                'terminal_type' => $shadow['terminal_type'] ?? null,
            ],
        ];
    }

    /**
     * @param  array<string, mixed>  $requestMeta
     * @param  array<string, mixed>  $primarySummary
     * @param  array<string, mixed>|null  $shadowSummary
     * @param  array<string, mixed>  $diff
     */
    private function persistShadowDiff(
        int $userId,
        string $threadId,
        array $requestMeta,
        array $primarySummary,
        ?array $shadowSummary,
        array $diff,
        string $status,
    ): void {
        DB::table('runtime_shadow_diffs')->insert([
            'id' => (string) Str::uuid(),
            'thread_id' => $threadId,
            'user_id' => $userId,
            'runtime_driver' => $this->primaryClient->driverName(),
            'shadow_driver' => $this->shadowClient->driverName(),
            'status' => $status,
            'request_meta' => json_encode($requestMeta, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE),
            'primary_summary' => json_encode($primarySummary, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE),
            'shadow_summary' => $shadowSummary === null
                ? null
                : json_encode($shadowSummary, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE),
            'diff' => json_encode($diff, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE),
            'created_at' => now(),
            'updated_at' => now(),
        ]);
    }
}
