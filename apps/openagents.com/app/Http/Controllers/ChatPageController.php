<?php

namespace App\Http\Controllers;

use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;
use Inertia\Inertia;
use Inertia\Response;
use Laravel\Ai\Contracts\ConversationStore;

class ChatPageController extends Controller
{
    public function show(Request $request, ?string $conversationId = null): Response|RedirectResponse
    {
        $user = $request->user();

        if (! $user) {
            abort(401);
        }

        if ($conversationId === null) {
            $conversationId = resolve(ConversationStore::class)
                ->storeConversation($user->id, 'New conversation');

            return redirect()->route('chat', ['conversationId' => $conversationId]);
        }

        $conversation = DB::table('agent_conversations')
            ->where('id', $conversationId)
            ->where('user_id', $user->id)
            ->first();

        if (! $conversation) {
            abort(404);
        }

        $messages = DB::table('agent_conversation_messages')
            ->where('conversation_id', $conversationId)
            ->where('user_id', $user->id)
            ->orderBy('created_at')
            ->get(['id', 'role', 'content'])
            ->map(fn ($m) => [
                'id' => $m->id,
                'role' => $m->role,
                'content' => $m->content,
            ])
            ->all();

        return Inertia::render('chat', [
            'conversationId' => $conversationId,
            'conversationTitle' => $conversation->title,
            'initialMessages' => $messages,
        ]);
    }
}
