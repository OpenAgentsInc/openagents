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

        if ($this->isGuestConversationId($existing)) {
            $existingId = strtolower(trim((string) $existing));

            if ($this->isConversationUsableForGuest($existingId)) {
                return $existingId;
            }
        }

        if ($this->isGuestConversationId($requestedConversationId)) {
            $requestedId = strtolower(trim((string) $requestedConversationId));

            if ($this->isConversationUsableForGuest($requestedId)) {
                $request->session()->put('chat.guest.conversation_id', $requestedId);

                return $requestedId;
            }
        }

        $id = $this->generateGuestConversationId();
        $request->session()->put('chat.guest.conversation_id', $id);

        return $id;
    }

    public function isGuestConversationId(mixed $value): bool
    {
        if (! is_string($value)) {
            return false;
        }

        $candidate = strtolower(trim($value));

        if ($candidate === '') {
            return false;
        }

        // Keep ids <= 36 chars to fit `agent_conversations.id` and `threads.id` schema.
        // Format: g- + 32 lowercase hex chars (34 chars total).
        return (bool) preg_match('/^g-[a-f0-9]{32}$/', $candidate);
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
            DB::table('agent_conversations')->insertOrIgnore([
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
            DB::table('threads')->insertOrIgnore([
                'id' => $conversationId,
                'user_id' => $userId,
                'autopilot_id' => null,
                'title' => 'Chat',
                'created_at' => $now,
                'updated_at' => $now,
            ]);
        }
    }

    public function guestOwnsConversation(string $conversationId): bool
    {
        $guestId = (int) $this->guestUser()->getAuthIdentifier();

        return DB::table('agent_conversations')
            ->where('id', $conversationId)
            ->where('user_id', $guestId)
            ->exists();
    }

    public function rotateGuestConversationId(Request $request): string
    {
        $id = $this->generateGuestConversationId();
        $request->session()->put('chat.guest.conversation_id', $id);

        return $id;
    }

    private function isConversationUsableForGuest(string $conversationId): bool
    {
        /** @var int|null $ownerIdRaw */
        $ownerIdRaw = DB::table('agent_conversations')
            ->where('id', $conversationId)
            ->value('user_id');

        // No owner yet means the id is safe for guest bootstrap.
        if ($ownerIdRaw === null) {
            return true;
        }

        $ownerId = (int) $ownerIdRaw;
        $guestId = (int) $this->guestUser()->getAuthIdentifier();

        return $ownerId > 0 && $ownerId === $guestId;
    }

    private function generateGuestConversationId(): string
    {
        $uuid = str_replace('-', '', strtolower((string) Str::uuid7()));

        return 'g-'.substr($uuid, 0, 32);
    }
}
