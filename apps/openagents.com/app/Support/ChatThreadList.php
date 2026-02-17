<?php

namespace App\Support;

use Illuminate\Support\Collection;
use Illuminate\Support\Facades\DB;

class ChatThreadList
{
    /**
     * @return Collection<int, object{id:string,title:string,created_at:mixed,updated_at:mixed}>
     */
    public function forUser(int $userId, int $limit = 50): Collection
    {
        $limit = max(1, min(200, $limit));

        $hasMessages = function ($query) use ($userId): void {
            $query->selectRaw('1')
                ->from('messages as m')
                ->whereColumn('m.thread_id', 'threads.id')
                ->where('m.user_id', $userId);
        };

        $nonEmpty = DB::table('threads')
            ->where('user_id', $userId)
            ->whereExists($hasMessages)
            ->orderByDesc('updated_at')
            ->limit($limit)
            ->get(['id', 'title', 'created_at', 'updated_at']);

        $latestEmpty = DB::table('threads')
            ->where('user_id', $userId)
            ->whereNotExists($hasMessages)
            ->orderByDesc('created_at')
            ->limit(1)
            ->get(['id', 'title', 'created_at', 'updated_at']);

        return $nonEmpty
            ->concat($latestEmpty)
            ->unique('id')
            ->sortByDesc(fn ($thread) => $thread->updated_at ?? $thread->created_at)
            ->take($limit)
            ->values();
    }

    public function normalizeTitle(?string $title): string
    {
        $trimmed = trim((string) $title);

        return $trimmed !== '' ? $trimmed : 'New conversation';
    }
}
