<?php

namespace App\Http\Controllers;

use App\Services\ShoutsService;
use Carbon\CarbonImmutable;
use Illuminate\Http\Request;
use Inertia\Inertia;
use Inertia\Response;

class FeedPageController extends Controller
{
    public function index(Request $request, ShoutsService $shouts): Response
    {
        $validated = $request->validate([
            'zone' => ['nullable', 'string', 'max:64', 'regex:/^[a-z0-9:_-]+$/i'],
            'limit' => ['nullable', 'integer', 'min:1'],
            'since' => ['nullable', 'date'],
        ]);

        $rawZone = isset($validated['zone']) ? strtolower(trim((string) $validated['zone'])) : null;
        $zone = ($rawZone === null || $rawZone === '' || $rawZone === 'all')
            ? null
            : $rawZone;

        $limit = max(1, min(200, (int) ($validated['limit'] ?? 50)));
        $since = isset($validated['since']) && is_string($validated['since'])
            ? CarbonImmutable::parse($validated['since'])
            : null;

        $rows = $shouts
            ->list($zone, $limit, null, $since)
            ->map(function ($shout): array {
                return [
                    'id' => (int) $shout->id,
                    'zone' => is_string($shout->zone) && trim($shout->zone) !== ''
                        ? (string) $shout->zone
                        : 'global',
                    'body' => (string) $shout->body,
                    'visibility' => (string) $shout->visibility,
                    'author' => [
                        'id' => (int) $shout->author->id,
                        'name' => (string) $shout->author->name,
                        'handle' => (string) $shout->author->handle,
                        'avatar' => $shout->author->avatar,
                    ],
                    'createdAt' => $shout->created_at?->toISOString(),
                    'updatedAt' => $shout->updated_at?->toISOString(),
                ];
            })
            ->values()
            ->all();

        return Inertia::render('feed', [
            'feed' => [
                'zone' => $zone,
                'limit' => $limit,
                'items' => $rows,
                'zones' => $shouts->topZones(20),
            ],
        ]);
    }
}
