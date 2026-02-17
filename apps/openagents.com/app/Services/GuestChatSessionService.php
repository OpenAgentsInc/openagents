<?php

namespace App\Services;

use App\Models\User;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;

class GuestChatSessionService
{
    private const GUEST_EMAIL = 'guest@openagents.internal';

    public function ensureGuestConversationId(Request $request, ?string $requestedConversationId = null): string
    {
        $existing = $request->session()->get('chat.guest.conversation_id');

        if (is_string($existing) && trim($existing) !== '') {
            return $existing;
        }

        if ($this->isGuestConversationId($requestedConversationId)) {
            $request->session()->put('chat.guest.conversation_id', $requestedConversationId);

            return $requestedConversationId;
        }

        $id = 'guest-'.Str::uuid7();
        $request->session()->put('chat.guest.conversation_id', $id);

        return $id;
    }

    public function isGuestConversationId(?string $value): bool
    {
        if (! is_string($value)) {
            return false;
        }

        $candidate = trim($value);

        if ($candidate === '') {
            return false;
        }

        return (bool) preg_match('/^guest-[a-z0-9-]+$/i', $candidate);
    }

    public function guestUser(): User
    {
        return User::firstOrCreate(
            ['email' => self::GUEST_EMAIL],
            [
                'name' => 'Guest',
                'workos_id' => 'guest-system',
                'avatar' => '',
            ]
        );
    }

    public function ensureGuestConversationAndThread(string $conversationId): void
    {
        $user = $this->guestUser();
        $userId = (int) $user->getAuthIdentifier();

        $exists = DB::table('agent_conversations')
            ->where('id', $conversationId)
            ->where('user_id', $userId)
            ->exists();

        if (! $exists) {
            $now = now();
            DB::table('agent_conversations')->insert([
                'id' => $conversationId,
                'user_id' => $userId,
                'title' => 'Chat',
                'created_at' => $now,
                'updated_at' => $now,
            ]);
        }

        $threadExists = DB::table('threads')
            ->where('id', $conversationId)
            ->where('user_id', $userId)
            ->exists();

        if (! $threadExists) {
            $now = now();
            DB::table('threads')->insert([
                'id' => $conversationId,
                'user_id' => $userId,
                'autopilot_id' => null,
                'title' => 'Chat',
                'created_at' => $now,
                'updated_at' => $now,
            ]);
        }
    }
}
