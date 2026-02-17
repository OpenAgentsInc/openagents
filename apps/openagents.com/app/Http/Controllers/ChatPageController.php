<?php

namespace App\Http\Controllers;

use App\Services\PostHogService;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;
use Inertia\Inertia;
use Inertia\Response;
use Laravel\Ai\Contracts\ConversationStore;

class ChatPageController extends Controller
{
    public function show(Request $request, ?string $conversationId = null): Response|RedirectResponse
    {
        $user = $request->user();

        if (! $user) {
            $guestConversationId = $this->ensureGuestConversationId($request);

            if ($conversationId === null || trim($conversationId) === '') {
                return redirect()->route('chat', ['conversationId' => $guestConversationId]);
            }

            if ($conversationId !== $guestConversationId) {
                return redirect()->route('chat', ['conversationId' => $guestConversationId]);
            }

            /** @var array{email?: string}|null $pending */
            $pending = $request->session()->get('auth.magic_auth');
            $pendingEmail = is_array($pending) && is_string($pending['email'] ?? null)
                ? trim((string) $pending['email'])
                : null;

            $guestStep = $pendingEmail ? 'code' : 'email';

            $initialAssistant = $pendingEmail
                ? "Check {$pendingEmail}. Enter your 6-digit verification code to continue setup."
                : "Welcome to Autopilot. To set up your agent, enter your email and I'll send a one-time code.";

            return Inertia::render('chat', [
                'conversationId' => $guestConversationId,
                'conversationTitle' => 'New conversation',
                'initialMessages' => [
                    [
                        'id' => (string) Str::uuid7(),
                        'role' => 'assistant',
                        'content' => $initialAssistant,
                    ],
                ],
                'guestOnboarding' => [
                    'enabled' => true,
                    'step' => $guestStep,
                    'pendingEmail' => $pendingEmail,
                ],
            ]);
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

            // PostHog: Track new chat started
            $posthog = resolve(PostHogService::class);
            $posthog->capture($user->email, 'chat started', [
                'conversation_id' => $conversationId,
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

        return Inertia::render('chat', [
            'conversationId' => $conversationId,
            'conversationTitle' => $thread->title,
            'initialMessages' => $messages,
            'guestOnboarding' => [
                'enabled' => false,
                'step' => null,
                'pendingEmail' => null,
            ],
        ]);
    }

    private function ensureGuestConversationId(Request $request): string
    {
        $existing = $request->session()->get('chat.guest.conversation_id');

        if (is_string($existing) && trim($existing) !== '') {
            return $existing;
        }

        $id = 'guest-'.Str::uuid7();
        $request->session()->put('chat.guest.conversation_id', $id);

        return $id;
    }
}
