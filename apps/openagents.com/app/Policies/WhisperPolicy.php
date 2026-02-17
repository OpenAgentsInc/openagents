<?php

namespace App\Policies;

use App\Models\User;
use App\Models\Whisper;

class WhisperPolicy
{
    public function view(User $actor, Whisper $whisper): bool
    {
        return (int) $whisper->sender_id === (int) $actor->id
            || (int) $whisper->recipient_id === (int) $actor->id;
    }

    public function update(User $actor, Whisper $whisper): bool
    {
        return (int) $whisper->recipient_id === (int) $actor->id;
    }
}
