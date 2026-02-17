<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\OpenApi\Parameters\LimitQueryParameter;
use App\OpenApi\RequestBodies\CreateAutopilotRequestBody;
use App\OpenApi\RequestBodies\CreateAutopilotThreadRequestBody;
use App\OpenApi\RequestBodies\UpdateAutopilotRequestBody;
use App\OpenApi\Responses\AutopilotListResponse;
use App\OpenApi\Responses\AutopilotResponse;
use App\OpenApi\Responses\AutopilotThreadListResponse;
use App\OpenApi\Responses\AutopilotThreadResponse;
use App\OpenApi\Responses\NotFoundResponse;
use App\OpenApi\Responses\UnauthorizedResponse;
use App\OpenApi\Responses\ValidationErrorResponse;
use App\Support\ChatThreadList;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;
use Laravel\Ai\Contracts\ConversationStore;
use Vyuldashev\LaravelOpenApi\Attributes as OpenApi;

#[OpenApi\PathItem]
class AutopilotController extends Controller
{
    private const DEFAULT_AUTOPILOT_ID = 'default';

    /**
     * List autopilots owned by the authenticated user.
     */
    #[OpenApi\Operation(tags: ['Autopilot'])]
    #[OpenApi\Response(factory: AutopilotListResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    public function index(Request $request): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        return response()->json([
            'data' => [
                $this->autopilotPayload($user),
            ],
        ]);
    }

    /**
     * Create an autopilot resource.
     *
     * Phase A behavior: idempotently returns the default autopilot skeleton.
     */
    #[OpenApi\Operation(tags: ['Autopilot'])]
    #[OpenApi\RequestBody(factory: CreateAutopilotRequestBody::class)]
    #[OpenApi\Response(factory: AutopilotResponse::class, statusCode: 201)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function store(Request $request): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $request->validate([
            'handle' => ['nullable', 'string', 'max:64'],
            'displayName' => ['nullable', 'string', 'max:120'],
        ]);

        return response()->json([
            'data' => $this->autopilotPayload($user),
        ], 201);
    }

    /**
     * Read one autopilot by id or handle.
     */
    #[OpenApi\Operation(tags: ['Autopilot'])]
    #[OpenApi\Response(factory: AutopilotResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    public function show(Request $request, string $autopilot): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $this->assertDefaultAutopilot($autopilot);

        return response()->json([
            'data' => $this->autopilotPayload($user),
        ]);
    }

    /**
     * Update one autopilot.
     *
     * Phase A behavior: validates input and returns the default autopilot skeleton.
     */
    #[OpenApi\Operation(tags: ['Autopilot'])]
    #[OpenApi\RequestBody(factory: UpdateAutopilotRequestBody::class)]
    #[OpenApi\Response(factory: AutopilotResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function update(Request $request, string $autopilot): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $this->assertDefaultAutopilot($autopilot);

        $request->validate([
            'displayName' => ['nullable', 'string', 'max:120'],
            'status' => ['nullable', 'string', 'in:active,disabled,archived'],
            'visibility' => ['nullable', 'string', 'in:private,discoverable,public'],
        ]);

        return response()->json([
            'data' => $this->autopilotPayload($user),
        ]);
    }

    /**
     * Create a new thread for an autopilot.
     */
    #[OpenApi\Operation(tags: ['Autopilot'])]
    #[OpenApi\RequestBody(factory: CreateAutopilotThreadRequestBody::class)]
    #[OpenApi\Response(factory: AutopilotThreadResponse::class, statusCode: 201)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function storeThread(Request $request, string $autopilot, ConversationStore $conversationStore): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $this->assertDefaultAutopilot($autopilot);

        $validated = $request->validate([
            'title' => ['nullable', 'string', 'max:200'],
        ]);

        $title = trim((string) ($validated['title'] ?? ''));
        if ($title === '') {
            $title = 'New conversation';
        }

        $conversationId = (string) $conversationStore->storeConversation($user->id, $title);
        $now = now();

        DB::table('threads')->insert([
            'id' => $conversationId,
            'user_id' => $user->id,
            'title' => $title,
            'created_at' => $now,
            'updated_at' => $now,
        ]);

        return response()->json([
            'data' => [
                'id' => $conversationId,
                'autopilotId' => self::DEFAULT_AUTOPILOT_ID,
                'title' => $title,
                'createdAt' => $now->toISOString(),
                'updatedAt' => $now->toISOString(),
            ],
        ], 201);
    }

    /**
     * List threads for one autopilot.
     */
    #[OpenApi\Operation(tags: ['Autopilot'])]
    #[OpenApi\Parameters(factory: LimitQueryParameter::class)]
    #[OpenApi\Response(factory: AutopilotThreadListResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    public function threads(Request $request, string $autopilot, ChatThreadList $threadList): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $this->assertDefaultAutopilot($autopilot);

        $limit = max(1, min(200, (int) $request->integer('limit', 50)));

        $threads = $threadList->forUser((int) $user->id, $limit)
            ->map(fn ($thread): array => [
                'id' => (string) $thread->id,
                'autopilotId' => self::DEFAULT_AUTOPILOT_ID,
                'title' => $threadList->normalizeTitle($thread->title),
                'createdAt' => $thread->created_at,
                'updatedAt' => $thread->updated_at,
            ])
            ->values()
            ->all();

        return response()->json([
            'data' => $threads,
        ]);
    }

    private function assertDefaultAutopilot(string $autopilot): void
    {
        $value = strtolower(trim($autopilot));
        if ($value !== self::DEFAULT_AUTOPILOT_ID) {
            abort(404);
        }
    }

    /**
     * @return array<string, mixed>
     */
    private function autopilotPayload(object $user): array
    {
        return [
            'id' => self::DEFAULT_AUTOPILOT_ID,
            'handle' => self::DEFAULT_AUTOPILOT_ID,
            'displayName' => 'Autopilot',
            'status' => 'active',
            'visibility' => 'private',
            'ownerUserId' => (int) $user->id,
            'createdAt' => $user->created_at?->toISOString(),
            'updatedAt' => $user->updated_at?->toISOString(),
            'phase' => 'phase_a_skeleton',
        ];
    }
}
