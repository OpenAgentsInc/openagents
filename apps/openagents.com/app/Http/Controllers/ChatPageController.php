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

            $now = now();

            DB::table('threads')->insert([
                'id' => $conversationId,
                'user_id' => $user->id,
                'title' => 'New conversation',
                'created_at' => $now,
                'updated_at' => $now,
            ]);

            return redirect()->route('chat', ['conversationId' => $conversationId]);
        }

        $conversation = DB::table('agent_conversations')
            ->where('id', $conversationId)
            ->where('user_id', $user->id)
            ->first();

        if (! $conversation) {
            abort(404);
        }

        $thread = DB::table('threads')
            ->where('id', $conversationId)
            ->where('user_id', $user->id)
            ->first();

        if (! $thread) {
            $now = now();

            DB::table('threads')->insert([
                'id' => $conversationId,
                'user_id' => $user->id,
                'title' => (string) $conversation->title,
                'created_at' => $now,
                'updated_at' => $now,
            ]);

            $thread = (object) [
                'id' => $conversationId,
                'title' => (string) $conversation->title,
            ];
        }

        $messages = DB::table('messages')
            ->where('thread_id', $conversationId)
            ->where('user_id', $user->id)
            ->orderBy('created_at')
            ->get(['id', 'role', 'content'])
            ->map(fn ($m) => [
                'id' => $m->id,
                'role' => $m->role,
                'content' => $m->content,
            ])
            ->all();

        // Backfill from laravel/ai conversation persistence (Phase 1) if needed.
        if (count($messages) === 0) {
            $legacy = DB::table('agent_conversation_messages')
                ->where('conversation_id', $conversationId)
                ->where('user_id', $user->id)
                ->orderBy('created_at')
                ->get(['id', 'role', 'content', 'created_at', 'updated_at']);

            foreach ($legacy as $m) {
                DB::table('messages')->updateOrInsert([
                    'id' => $m->id,
                ], [
                    'thread_id' => $conversationId,
                    'run_id' => null,
                    'user_id' => $user->id,
                    'role' => $m->role,
                    'content' => $m->content,
                    'meta' => null,
                    'created_at' => $m->created_at,
                    'updated_at' => $m->updated_at,
                ]);
            }

            $messages = DB::table('messages')
                ->where('thread_id', $conversationId)
                ->where('user_id', $user->id)
                ->orderBy('created_at')
                ->get(['id', 'role', 'content'])
                ->map(fn ($m) => [
                    'id' => $m->id,
                    'role' => $m->role,
                    'content' => $m->content,
                ])
                ->all();
        }

        $runs = DB::table('runs')
            ->where('thread_id', $conversationId)
            ->where('user_id', $user->id)
            ->orderByDesc('created_at')
            ->limit(20)
            ->get(['id', 'status', 'model_provider', 'model', 'started_at', 'completed_at', 'created_at'])
            ->map(fn ($r) => [
                'id' => $r->id,
                'status' => $r->status,
                'modelProvider' => $r->model_provider,
                'model' => $r->model,
                'startedAt' => $r->started_at,
                'completedAt' => $r->completed_at,
                'createdAt' => $r->created_at,
            ])
            ->all();

        $selectedRunId = $runs[0]['id'] ?? null;

        $runEvents = [];
        if (is_string($selectedRunId)) {
            $runEvents = DB::table('run_events')
                ->where('run_id', $selectedRunId)
                ->where('user_id', $user->id)
                ->orderBy('id')
                ->get(['id', 'type', 'payload', 'created_at'])
                ->map(fn ($e) => [
                    'id' => $e->id,
                    'type' => $e->type,
                    'payload' => $e->payload,
                    'createdAt' => $e->created_at,
                ])
                ->all();
        }

        return Inertia::render('chat', [
            'conversationId' => $conversationId,
            'conversationTitle' => $thread->title,
            'initialMessages' => $messages,
            'runs' => $runs,
            'selectedRunId' => $selectedRunId,
            'runEvents' => $runEvents,
        ]);
    }
}
