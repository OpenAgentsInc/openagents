<?php

namespace App\Services;

use App\Models\Shout;
use App\Models\User;
use Carbon\CarbonImmutable;
use Illuminate\Database\Eloquent\Collection;
use Illuminate\Support\Facades\DB;

class ShoutsService
{
    public function create(User $actor, string $body, ?string $zone): Shout
    {
        $normalizedZone = $this->normalizeZone($zone);

        /** @var Shout $shout */
        $shout = Shout::query()->create([
            'user_id' => $actor->id,
            'zone' => $normalizedZone,
            'body' => trim($body),
            'visibility' => 'public',
        ]);

        return $shout->load('author');
    }

    /**
     * @return Collection<int, Shout>
     */
    public function list(?string $zone, int $limit, ?int $beforeId, ?CarbonImmutable $since): Collection
    {
        $query = Shout::query()
            ->with('author')
            ->orderByDesc('id');

        $normalizedZone = $this->normalizeZone($zone);
        if ($normalizedZone !== null) {
            $query->where('zone', $normalizedZone);
        }

        if ($beforeId !== null) {
            $query->where('id', '<', $beforeId);
        }

        if ($since !== null) {
            $query->where('created_at', '>=', $since);
        }

        return $query->limit($limit)->get();
    }

    /**
     * @return array<int, array{zone:string,count24h:int}>
     */
    public function topZones(int $limit = 20): array
    {
        return DB::table('shouts')
            ->selectRaw('zone, COUNT(*) as count24h')
            ->whereNotNull('zone')
            ->where('created_at', '>=', now()->subDay())
            ->groupBy('zone')
            ->orderByDesc('count24h')
            ->orderBy('zone')
            ->limit($limit)
            ->get()
            ->map(fn ($row): array => [
                'zone' => (string) $row->zone,
                'count24h' => (int) $row->count24h,
            ])
            ->all();
    }

    public function normalizeZone(?string $zone): ?string
    {
        if (! is_string($zone)) {
            return null;
        }

        $normalized = strtolower(trim($zone));

        return $normalized === '' ? null : $normalized;
    }
}
