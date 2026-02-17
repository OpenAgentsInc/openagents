<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Http\Requests\Api\CreateShoutRequest;
use App\Http\Resources\Api\ShoutResource;
use App\OpenApi\Parameters\LimitQueryParameter;
use App\OpenApi\Parameters\ShoutsIndexParameters;
use App\OpenApi\RequestBodies\CreateShoutRequestBody;
use App\OpenApi\Responses\ForbiddenResponse;
use App\OpenApi\Responses\ShoutListResponse;
use App\OpenApi\Responses\ShoutResponse;
use App\OpenApi\Responses\ShoutZonesResponse;
use App\OpenApi\Responses\UnauthorizedResponse;
use App\OpenApi\Responses\ValidationErrorResponse;
use App\Services\ShoutsService;
use Carbon\CarbonImmutable;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Vyuldashev\LaravelOpenApi\Attributes as OpenApi;

#[OpenApi\PathItem]
class ShoutsController extends Controller
{
    /**
     * List public shouts.
     *
     * Returns newest-first shout messages. Optionally filter by zone and since
     * timestamp, and paginate using `before_id`.
     */
    #[OpenApi\Operation(tags: ['Shouts'])]
    #[OpenApi\Parameters(factory: ShoutsIndexParameters::class)]
    #[OpenApi\Response(factory: ShoutListResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function index(Request $request, ShoutsService $service): JsonResponse
    {
        $validated = $request->validate([
            'zone' => ['nullable', 'string', 'max:64', 'regex:/^[a-z0-9:_-]+$/i'],
            'limit' => ['nullable', 'integer', 'min:1'],
            'before_id' => ['nullable', 'integer', 'min:1'],
            'since' => ['nullable', 'date'],
        ]);

        $limit = max(1, min(200, (int) ($validated['limit'] ?? 50)));
        $zone = isset($validated['zone']) ? (string) $validated['zone'] : null;
        $beforeId = isset($validated['before_id']) ? (int) $validated['before_id'] : null;
        $since = isset($validated['since']) && is_string($validated['since'])
            ? CarbonImmutable::parse($validated['since'])
            : null;

        $shouts = $service->list($zone, $limit, $beforeId, $since);

        return response()->json([
            'data' => ShoutResource::collection($shouts)->resolve(),
            'meta' => [
                'nextCursor' => $shouts->count() === $limit ? (string) optional($shouts->last())->id : null,
            ],
        ]);
    }

    /**
     * Create a shout.
     */
    #[OpenApi\Operation(tags: ['Shouts'])]
    #[OpenApi\RequestBody(factory: CreateShoutRequestBody::class)]
    #[OpenApi\Response(factory: ShoutResponse::class, statusCode: 201)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ForbiddenResponse::class, statusCode: 403)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function store(CreateShoutRequest $request, ShoutsService $service): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $validated = $request->validated();
        $shout = $service->create(
            $user,
            (string) $validated['body'],
            $validated['zone'] ?? null,
        );

        return response()->json([
            'data' => (new ShoutResource($shout))->resolve(),
        ], 201);
    }

    /**
     * Return top shout zones for discovery.
     */
    #[OpenApi\Operation(tags: ['Shouts'])]
    #[OpenApi\Parameters(factory: LimitQueryParameter::class)]
    #[OpenApi\Response(factory: ShoutZonesResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function zones(Request $request, ShoutsService $service): JsonResponse
    {
        $validated = $request->validate([
            'limit' => ['nullable', 'integer', 'min:1'],
        ]);

        $limit = max(1, min(100, (int) ($validated['limit'] ?? 20)));

        return response()->json([
            'data' => $service->topZones($limit),
        ]);
    }
}
