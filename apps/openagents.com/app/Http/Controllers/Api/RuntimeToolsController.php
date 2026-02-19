<?php

namespace App\Http\Controllers\Api;

use App\AI\Runtime\RuntimeToolsClient;
use App\Http\Controllers\Controller;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class RuntimeToolsController extends Controller
{
    public function execute(Request $request, RuntimeToolsClient $runtimeToolsClient): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $validated = $request->validate([
            'tool_pack' => ['required', 'string', 'max:120'],
            'mode' => ['nullable', 'string', 'in:execute,replay'],
            'manifest' => ['nullable', 'array'],
            'manifest_ref' => ['nullable', 'array'],
            'request' => ['required', 'array'],
            'policy' => ['nullable', 'array'],
            'run_id' => ['nullable', 'string', 'max:160'],
            'thread_id' => ['nullable', 'string', 'max:160'],
            'user_id' => ['nullable', 'integer', 'min:1'],
        ]);

        if (! isset($validated['manifest']) && ! isset($validated['manifest_ref'])) {
            return response()->json([
                'error' => [
                    'code' => 'invalid_request',
                    'message' => 'manifest or manifest_ref is required',
                ],
            ], 422);
        }

        $authenticatedUserId = (int) $user->getAuthIdentifier();
        $requestedUserId = isset($validated['user_id']) ? (int) $validated['user_id'] : null;

        if ($requestedUserId !== null && $requestedUserId !== $authenticatedUserId) {
            return response()->json([
                'error' => [
                    'code' => 'forbidden',
                    'message' => 'user_id does not match authenticated principal',
                ],
            ], 403);
        }

        $payload = [
            'tool_pack' => (string) $validated['tool_pack'],
            'mode' => isset($validated['mode']) ? (string) $validated['mode'] : 'execute',
            'manifest' => $validated['manifest'] ?? [],
            'manifest_ref' => $validated['manifest_ref'] ?? [],
            'request' => $validated['request'],
            'policy' => $validated['policy'] ?? [],
            'run_id' => $validated['run_id'] ?? null,
            'thread_id' => $validated['thread_id'] ?? null,
            'user_id' => $authenticatedUserId,
        ];

        $payload['request']['user_id'] = $authenticatedUserId;
        if (is_string($payload['run_id']) && trim($payload['run_id']) !== '' && ! isset($payload['request']['run_id'])) {
            $payload['request']['run_id'] = $payload['run_id'];
        }
        if (is_string($payload['thread_id']) && trim($payload['thread_id']) !== '' && ! isset($payload['request']['thread_id'])) {
            $payload['request']['thread_id'] = $payload['thread_id'];
        }

        $result = $runtimeToolsClient->execute($payload, [
            'run_id' => $payload['run_id'],
            'thread_id' => $payload['thread_id'],
            'user_id' => $authenticatedUserId,
        ]);

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
                'code' => 'runtime_tools_failed',
                'message' => (string) ($result['error'] ?? 'runtime tools request failed'),
            ],
        ], $status);
    }
}
