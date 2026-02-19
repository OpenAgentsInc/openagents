<?php

namespace App\Http\Controllers\Api\Internal;

use App\Http\Controllers\Controller;
use App\Models\UserIntegration;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class RuntimeSecretController extends Controller
{
    public function fetch(Request $request): JsonResponse
    {
        $validated = $request->validate([
            'user_id' => ['required', 'integer', 'min:1'],
            'provider' => ['required', 'string', 'in:resend'],
            'integration_id' => ['required', 'string', 'max:160'],
            'run_id' => ['required', 'string', 'max:160'],
            'tool_call_id' => ['required', 'string', 'max:160'],
            'org_id' => ['nullable', 'string', 'max:160'],
        ]);

        $integration = UserIntegration::query()
            ->where('user_id', (int) $validated['user_id'])
            ->where('provider', (string) $validated['provider'])
            ->where('status', 'active')
            ->first();

        if (! $integration || ! is_string($integration->encrypted_secret) || trim($integration->encrypted_secret) === '') {
            return response()->json([
                'error' => [
                    'code' => 'secret_not_found',
                    'message' => 'active provider secret not found',
                ],
            ], 404);
        }

        return response()->json([
            'data' => [
                'provider' => (string) $integration->provider,
                'secret' => (string) $integration->encrypted_secret,
                'cache_ttl_ms' => (int) config('runtime.internal.secret_cache_ttl_ms', 60000),
                'scope' => [
                    'user_id' => (int) $validated['user_id'],
                    'provider' => (string) $validated['provider'],
                    'integration_id' => (string) $validated['integration_id'],
                    'run_id' => (string) $validated['run_id'],
                    'tool_call_id' => (string) $validated['tool_call_id'],
                    'org_id' => $validated['org_id'] ?? null,
                ],
                'fetched_at' => now()->toISOString(),
            ],
        ]);
    }
}
