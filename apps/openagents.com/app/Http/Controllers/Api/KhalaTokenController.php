<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Support\Khala\KhalaTokenIssuer;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use RuntimeException;

class KhalaTokenController extends Controller
{
    public function store(Request $request, KhalaTokenIssuer $issuer): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $validated = $request->validate([
            'scope' => ['nullable', 'array'],
            'scope.*' => ['string', 'max:120'],
            'workspace_id' => ['nullable', 'string', 'max:120'],
            'role' => ['nullable', 'string', 'in:member,admin,owner'],
        ]);

        try {
            $token = $issuer->issueForUser($user, $validated['scope'] ?? [], [
                'workspace_id' => $validated['workspace_id'] ?? null,
                'role' => $validated['role'] ?? null,
            ]);
        } catch (RuntimeException $exception) {
            return response()->json([
                'error' => [
                    'code' => 'khala_token_unavailable',
                    'message' => $exception->getMessage(),
                ],
            ], 503);
        }

        return response()->json(['data' => $token]);
    }
}
