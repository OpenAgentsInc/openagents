<?php

namespace App\Services;

use App\Models\Autopilot;
use App\Models\Thread;
use App\Models\User;
use Illuminate\Database\Eloquent\ModelNotFoundException;
use Illuminate\Support\Facades\DB;
use Laravel\Ai\Contracts\ConversationStore;

class AutopilotThreadService
{
    public function __construct(
        private readonly ConversationStore $conversationStore,
    ) {}

    public function ensureThread(User $user, Autopilot $autopilot, ?string $conversationId = null, ?string $title = null): Thread
    {
        $conversationId = is_string($conversationId) ? trim($conversationId) : '';

        if ($conversationId === '') {
            $normalizedTitle = $this->normalizeTitle($title);
            $conversationId = (string) $this->conversationStore->storeConversation($user->id, $normalizedTitle);
        }

        $conversation = DB::table('agent_conversations')
            ->where('id', $conversationId)
            ->where('user_id', $user->id)
            ->first();

        if (! $conversation) {
            throw (new ModelNotFoundException)->setModel('agent_conversations', [$conversationId]);
        }

        $thread = Thread::query()
            ->where('id', $conversationId)
            ->where('user_id', $user->id)
            ->first();

        if ($thread) {
            if (is_string($thread->autopilot_id) && $thread->autopilot_id !== '' && $thread->autopilot_id !== $autopilot->id) {
                throw (new ModelNotFoundException)->setModel(Thread::class, [$conversationId]);
            }

            $didChange = false;

            if ($thread->autopilot_id !== $autopilot->id) {
                $thread->autopilot_id = $autopilot->id;
                $didChange = true;
            }

            if (! is_string($thread->title) || trim($thread->title) === '') {
                $thread->title = $this->normalizeTitle($title ?: (string) ($conversation->title ?? ''));
                $didChange = true;
            }

            if ($didChange) {
                $thread->save();
            }

            return $thread;
        }

        $resolvedTitle = $this->normalizeTitle($title ?: (string) ($conversation->title ?? ''));

        return Thread::query()->create([
            'id' => $conversationId,
            'user_id' => $user->id,
            'autopilot_id' => $autopilot->id,
            'title' => $resolvedTitle,
            'created_at' => now(),
            'updated_at' => now(),
        ]);
    }

    private function normalizeTitle(?string $title): string
    {
        $candidate = is_string($title) ? trim($title) : '';
        if ($candidate === '') {
            return 'New conversation';
        }

        return mb_substr($candidate, 0, 200);
    }
}
