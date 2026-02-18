<?php

namespace App\Http\Controllers;

use App\Services\GuestChatSessionService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Throwable;

class GuestChatSessionController extends Controller
{
    public function __invoke(Request $request, GuestChatSessionService $guestService): JsonResponse
    {
        try {
            // If user is already authenticated (e.g. just logged in via chat), do not create
            // guest conversation/thread — the conversation may already be adopted. Just echo
            // the requested id so the client gets a valid JSON response.
            if ($request->user()) {
                $requested = $request->query('conversationId');
                $id = is_string($requested) && $guestService->isGuestConversationId($requested)
                    ? strtolower(trim($requested))
                    : null;

                return response()->json([
                    'conversationId' => $id ?? $request->session()->get('chat.guest.conversation_id'),
                ]);
            }

            $requestedConversationId = $request->query('conversationId');
            $conversationId = $guestService->ensureGuestConversationId(
                $request,
                is_string($requestedConversationId) ? $requestedConversationId : null,
            );

            $guestService->ensureGuestConversationAndThread($conversationId);

            return response()->json([
                'conversationId' => $conversationId,
            ]);
        } catch (Throwable $e) {
            report($e);

            return response()->json([
                'message' => 'Unable to establish guest session.',
                'conversationId' => null,
            ], 500);
        }
    }
}
