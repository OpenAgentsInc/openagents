<?php

namespace App\Http\Controllers;

use App\Services\GuestChatSessionService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class GuestChatSessionController extends Controller
{
    public function __invoke(Request $request, GuestChatSessionService $guestService): JsonResponse
    {
        $requestedConversationId = $request->query('conversationId');
        $conversationId = $guestService->ensureGuestConversationId(
            $request,
            is_string($requestedConversationId) ? $requestedConversationId : null,
        );

        $guestService->ensureGuestConversationAndThread($conversationId);

        return response()->json([
            'conversationId' => $conversationId,
        ]);
    }
}
