<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Support\Convex\ConvexTokenIssuer;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use RuntimeException;

class ConvexTokenController extends Controller
{
    public function store(Request $request, ConvexTokenIssuer $issuer): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $validated = $request->validate([
            'scope' => ['nullable', 'array'],
            'scope.*' => ['string', 'max:120'],
        ]);

        try {
            $token = $issuer->issueForUser($user, $validated['scope'] ?? []);
        } catch (RuntimeException $exception) {
            return response()->json([
                'error' => [
                    'code' => 'convex_token_unavailable',
                    'message' => $exception->getMessage(),
                ],
            ], 503);
        }

        return response()->json(['data' => $token]);
    }
}
