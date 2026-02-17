<?php

namespace App\Services;

use App\Models\User;
use App\Models\Whisper;
use Illuminate\Database\Eloquent\Collection;

class WhispersService
{
    public function send(User $sender, User $recipient, string $body): Whisper
    {
        /** @var Whisper $whisper */
        $whisper = Whisper::query()->create([
            'sender_id' => $sender->id,
            'recipient_id' => $recipient->id,
            'body' => trim($body),
        ]);

        return $whisper->load(['sender', 'recipient']);
    }

    /**
     * @return Collection<int, Whisper>
     */
    public function listFor(User $actor, ?User $withUser, int $limit, ?int $beforeId): Collection
    {
        $query = Whisper::query()
            ->with(['sender', 'recipient'])
            ->orderByDesc('id');

        if ($withUser) {
            $query->where(function ($inner) use ($actor, $withUser): void {
                $inner->where(function ($pair) use ($actor, $withUser): void {
                    $pair->where('sender_id', $actor->id)
                        ->where('recipient_id', $withUser->id);
                })->orWhere(function ($pair) use ($actor, $withUser): void {
                    $pair->where('sender_id', $withUser->id)
                        ->where('recipient_id', $actor->id);
                });
            });
        } else {
            $query->where(function ($inner) use ($actor): void {
                $inner->where('sender_id', $actor->id)
                    ->orWhere('recipient_id', $actor->id);
            });
        }

        if ($beforeId !== null) {
            $query->where('id', '<', $beforeId);
        }

        return $query->limit($limit)->get();
    }

    public function markRead(Whisper $whisper, User $recipient): Whisper
    {
        if ((int) $whisper->recipient_id !== (int) $recipient->id) {
            return $whisper;
        }

        if ($whisper->read_at === null) {
            $whisper->read_at = now();
            $whisper->save();
        }

        return $whisper->refresh()->load(['sender', 'recipient']);
    }
}
