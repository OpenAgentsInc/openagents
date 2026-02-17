<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Http\Requests\Api\CreateWhisperRequest;
use App\Http\Resources\Api\WhisperResource;
use App\Models\User;
use App\Models\Whisper;
use App\OpenApi\Parameters\WhispersIndexParameters;
use App\OpenApi\RequestBodies\CreateWhisperRequestBody;
use App\OpenApi\Responses\ForbiddenResponse;
use App\OpenApi\Responses\NotFoundResponse;
use App\OpenApi\Responses\UnauthorizedResponse;
use App\OpenApi\Responses\ValidationErrorResponse;
use App\OpenApi\Responses\WhisperListResponse;
use App\OpenApi\Responses\WhisperResponse;
use App\Services\WhispersService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Validation\ValidationException;
use Vyuldashev\LaravelOpenApi\Attributes as OpenApi;

#[OpenApi\PathItem]
class WhispersController extends Controller
{
    /**
     * List whispers for the authenticated user.
     *
     * Without `with`, returns a unified inbox/outbox stream. With `with`, returns
     * the direct thread between the actor and the referenced user.
     */
    #[OpenApi\Operation(tags: ['Whispers'])]
    #[OpenApi\Parameters(factory: WhispersIndexParameters::class)]
    #[OpenApi\Response(factory: WhisperListResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ForbiddenResponse::class, statusCode: 403)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    public function index(Request $request, WhispersService $service): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $this->ensureAbility($request, 'whispers:read');

        $validated = $request->validate([
            'with' => ['nullable', 'string', 'max:64'],
            'limit' => ['nullable', 'integer', 'min:1'],
            'before_id' => ['nullable', 'integer', 'min:1'],
        ]);

        $withInput = isset($validated['with']) ? strtolower(trim((string) $validated['with'])) : null;
        $withUser = null;

        if (is_string($withInput) && $withInput !== '') {
            $withUser = $this->resolveConversationPeer($withInput);
            if (! $withUser) {
                abort(404);
            }
        }

        $limit = max(1, min(200, (int) ($validated['limit'] ?? 50)));
        $beforeId = isset($validated['before_id']) ? (int) $validated['before_id'] : null;

        $whispers = $service->listFor($user, $withUser, $limit, $beforeId);

        return response()->json([
            'data' => WhisperResource::collection($whispers)->resolve(),
            'meta' => [
                'nextCursor' => $whispers->count() === $limit ? (string) optional($whispers->last())->id : null,
                'with' => $withUser?->handle,
            ],
        ]);
    }

    /**
     * Send a new whisper.
     */
    #[OpenApi\Operation(tags: ['Whispers'])]
    #[OpenApi\RequestBody(factory: CreateWhisperRequestBody::class)]
    #[OpenApi\Response(factory: WhisperResponse::class, statusCode: 201)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ForbiddenResponse::class, statusCode: 403)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    public function store(CreateWhisperRequest $request, WhispersService $service): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $validated = $request->validated();

        $recipient = null;

        if (isset($validated['recipientId'])) {
            $recipient = User::query()->find((int) $validated['recipientId']);
        } elseif (isset($validated['recipientHandle'])) {
            $recipient = User::query()->where('handle', strtolower((string) $validated['recipientHandle']))->first();
        }

        if (! $recipient) {
            abort(404);
        }

        if ((int) $recipient->id === (int) $user->id) {
            throw ValidationException::withMessages([
                'recipientId' => 'Cannot whisper yourself.',
            ]);
        }

        $whisper = $service->send($user, $recipient, (string) $validated['body']);

        return response()->json([
            'data' => (new WhisperResource($whisper))->resolve(),
        ], 201);
    }

    /**
     * Mark a whisper as read.
     */
    #[OpenApi\Operation(tags: ['Whispers'])]
    #[OpenApi\Response(factory: WhisperResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ForbiddenResponse::class, statusCode: 403)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    public function read(Request $request, int $id, WhispersService $service): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $this->ensureAbility($request, 'whispers:write');

        $whisper = Whisper::query()->with(['sender', 'recipient'])->find($id);
        if (! $whisper) {
            abort(404);
        }

        $this->authorize('update', $whisper);

        $updated = $service->markRead($whisper, $user);

        return response()->json([
            'data' => (new WhisperResource($updated))->resolve(),
        ]);
    }

    private function ensureAbility(Request $request, string $ability): void
    {
        $user = $request->user();
        $token = $user?->currentAccessToken();

        if ($token && ! $token->can('*') && ! $token->can($ability)) {
            abort(403);
        }
    }

    private function resolveConversationPeer(string $with): ?User
    {
        if ($with === '') {
            return null;
        }

        if (ctype_digit($with)) {
            return User::query()->find((int) $with);
        }

        if (! preg_match('/^[a-z0-9:_-]+$/', $with)) {
            throw ValidationException::withMessages([
                'with' => 'with must be a numeric id or a valid handle.',
            ]);
        }

        return User::query()->where('handle', strtolower($with))->first();
    }
}
