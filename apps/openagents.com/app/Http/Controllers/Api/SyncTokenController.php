<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Support\Sync\SyncTokenIssuer;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use InvalidArgumentException;
use RuntimeException;

class SyncTokenController extends Controller
{
    public function store(Request $request, SyncTokenIssuer $issuer): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $validated = $request->validate([
            'scopes' => ['nullable', 'array'],
            'scopes.*' => ['string', 'max:120'],
        ]);

        try {
            $token = $issuer->issueForUser($user, $validated['scopes'] ?? []);
        } catch (InvalidArgumentException $exception) {
            return response()->json([
                'error' => [
                    'code' => 'invalid_scope',
                    'message' => $exception->getMessage(),
                ],
            ], 422);
        } catch (RuntimeException $exception) {
            return response()->json([
                'error' => [
                    'code' => 'sync_token_unavailable',
                    'message' => $exception->getMessage(),
                ],
            ], 503);
        }

        return response()->json(['data' => $token]);
    }
}
