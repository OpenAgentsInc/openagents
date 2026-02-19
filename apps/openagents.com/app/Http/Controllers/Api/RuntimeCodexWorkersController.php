<?php

namespace App\Http\Controllers\Api;

use App\AI\Runtime\RuntimeCodexClient;
use App\Http\Controllers\Controller;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Symfony\Component\HttpFoundation\StreamedResponse;

class RuntimeCodexWorkersController extends Controller
{
    public function index(Request $request, RuntimeCodexClient $client): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $validated = $request->validate([
            'status' => ['nullable', 'in:starting,running,stopping,stopped,failed'],
            'workspace_ref' => ['nullable', 'string', 'max:255'],
            'limit' => ['nullable', 'integer', 'min:1', 'max:200'],
        ]);

        $path = (string) config('runtime.elixir.codex_workers_path', '/internal/v1/codex/workers');
        $query = [];

        if (array_key_exists('status', $validated) && is_string($validated['status'])) {
            $query['status'] = $validated['status'];
        }

        if (array_key_exists('workspace_ref', $validated) && is_string($validated['workspace_ref'])) {
            $query['workspace_ref'] = $validated['workspace_ref'];
        }

        if (array_key_exists('limit', $validated) && $validated['limit'] !== null) {
            $query['limit'] = (int) $validated['limit'];
        }

        if ($query !== []) {
            $path .= '?'.http_build_query($query);
        }

        $result = $client->request('GET', $path, null, [
            'user_id' => (int) $user->getAuthIdentifier(),
        ]);

        return $this->fromRuntimeResult($result);
    }

    public function create(Request $request, RuntimeCodexClient $client): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $validated = $request->validate([
            'worker_id' => ['nullable', 'string', 'max:160'],
            'workspace_ref' => ['nullable', 'string', 'max:255'],
            'codex_home_ref' => ['nullable', 'string', 'max:255'],
            'adapter' => ['nullable', 'string', 'max:120'],
            'metadata' => ['nullable', 'array'],
        ]);

        $path = (string) config('runtime.elixir.codex_workers_path', '/internal/v1/codex/workers');

        $result = $client->request('POST', $path, $validated, [
            'user_id' => (int) $user->getAuthIdentifier(),
        ]);

        return $this->fromRuntimeResult($result);
    }

    public function show(string $workerId, Request $request, RuntimeCodexClient $client): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $pathTemplate = (string) config('runtime.elixir.codex_worker_snapshot_path_template', '/internal/v1/codex/workers/{worker_id}/snapshot');
        $path = str_replace('{worker_id}', $workerId, $pathTemplate);

        $result = $client->request('GET', $path, null, [
            'user_id' => (int) $user->getAuthIdentifier(),
        ]);

        return $this->fromRuntimeResult($result);
    }

    public function request(string $workerId, Request $request, RuntimeCodexClient $client): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $validated = $request->validate([
            'request' => ['required', 'array'],
        ]);

        $pathTemplate = (string) config('runtime.elixir.codex_worker_requests_path_template', '/internal/v1/codex/workers/{worker_id}/requests');
        $path = str_replace('{worker_id}', $workerId, $pathTemplate);

        $result = $client->request('POST', $path, $validated, [
            'user_id' => (int) $user->getAuthIdentifier(),
        ]);

        return $this->fromRuntimeResult($result);
    }

    public function stop(string $workerId, Request $request, RuntimeCodexClient $client): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $validated = $request->validate([
            'reason' => ['nullable', 'string', 'max:255'],
        ]);

        $pathTemplate = (string) config('runtime.elixir.codex_worker_stop_path_template', '/internal/v1/codex/workers/{worker_id}/stop');
        $path = str_replace('{worker_id}', $workerId, $pathTemplate);

        $result = $client->request('POST', $path, $validated, [
            'user_id' => (int) $user->getAuthIdentifier(),
        ]);

        return $this->fromRuntimeResult($result);
    }

    public function stream(string $workerId, Request $request, RuntimeCodexClient $client): JsonResponse|StreamedResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $validated = $request->validate([
            'cursor' => ['nullable', 'integer', 'min:0'],
            'tail_ms' => ['nullable', 'integer', 'min:1', 'max:120000'],
        ]);

        $snapshotPathTemplate = (string) config('runtime.elixir.codex_worker_snapshot_path_template', '/internal/v1/codex/workers/{worker_id}/snapshot');
        $snapshotPath = str_replace('{worker_id}', $workerId, $snapshotPathTemplate);

        // Enforce ownership before opening long-lived SSE proxy.
        $snapshot = $client->request('GET', $snapshotPath, null, [
            'user_id' => (int) $user->getAuthIdentifier(),
        ]);

        if ($snapshot['ok'] !== true) {
            return $this->fromRuntimeResult($snapshot);
        }

        $streamPathTemplate = (string) config('runtime.elixir.codex_worker_stream_path_template', '/internal/v1/codex/workers/{worker_id}/stream');
        $streamPath = str_replace('{worker_id}', $workerId, $streamPathTemplate);

        $query = [];

        if (array_key_exists('cursor', $validated) && $validated['cursor'] !== null) {
            $query['cursor'] = (int) $validated['cursor'];
        }

        if (array_key_exists('tail_ms', $validated) && $validated['tail_ms'] !== null) {
            $query['tail_ms'] = (int) $validated['tail_ms'];
        }

        $lastEventId = $request->header('Last-Event-ID');
        if (! is_string($lastEventId) || trim($lastEventId) === '') {
            $lastEventId = null;
        }

        return $client->stream($streamPath, $query, [
            'user_id' => (int) $user->getAuthIdentifier(),
            'last_event_id' => $lastEventId,
        ]);
    }

    /**
     * @param  array{ok: bool, status: int|null, body: mixed, error: string|null}  $result
     */
    private function fromRuntimeResult(array $result): JsonResponse
    {
        if ($result['ok'] === true) {
            if (is_array($result['body'])) {
                return response()->json($result['body'], $result['status'] ?? 200);
            }

            return response()->json(['data' => ['raw' => (string) ($result['body'] ?? '')]], $result['status'] ?? 200);
        }

        $status = $result['status'] ?? 502;

        if (is_array($result['body'])) {
            return response()->json($result['body'], $status);
        }

        return response()->json([
            'error' => [
                'code' => 'runtime_codex_failed',
                'message' => (string) ($result['error'] ?? 'runtime codex request failed'),
            ],
        ], $status);
    }
}
