<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Models\Autopilot;
use App\Models\Thread;
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
use App\Services\AutopilotService;
use App\Services\AutopilotThreadService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Vyuldashev\LaravelOpenApi\Attributes as OpenApi;

#[OpenApi\PathItem]
class AutopilotController extends Controller
{
    /**
     * List autopilots owned by the authenticated user.
     */
    #[OpenApi\Operation(tags: ['Autopilot'])]
    #[OpenApi\Response(factory: AutopilotListResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    public function index(Request $request, AutopilotService $autopilotService): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $limit = max(1, min(200, (int) $request->integer('limit', 100)));

        $autopilots = $autopilotService
            ->listForUser($user, $limit)
            ->map(fn (Autopilot $autopilot): array => $this->autopilotPayload($autopilot))
            ->values()
            ->all();

        return response()->json([
            'data' => $autopilots,
        ]);
    }

    /**
     * Create an autopilot resource.
     */
    #[OpenApi\Operation(tags: ['Autopilot'])]
    #[OpenApi\RequestBody(factory: CreateAutopilotRequestBody::class)]
    #[OpenApi\Response(factory: AutopilotResponse::class, statusCode: 201)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function store(Request $request, AutopilotService $autopilotService): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $validated = $request->validate([
            'handle' => ['nullable', 'string', 'max:64', 'regex:/^[a-zA-Z0-9:_-]+$/'],
            'displayName' => ['nullable', 'string', 'max:120'],
            'avatar' => ['nullable', 'string', 'max:255'],
            'tagline' => ['nullable', 'string', 'max:255'],
            'status' => ['nullable', 'string', 'in:active,disabled,archived'],
            'visibility' => ['nullable', 'string', 'in:private,discoverable,public'],
        ]);

        $autopilot = $autopilotService->createForUser($user, $validated);

        return response()->json([
            'data' => $this->autopilotPayload($autopilot),
        ], 201);
    }

    /**
     * Read one autopilot by id or handle.
     */
    #[OpenApi\Operation(tags: ['Autopilot'])]
    #[OpenApi\Response(factory: AutopilotResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    public function show(Request $request, string $autopilot, AutopilotService $autopilotService): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $entity = $autopilotService->resolveOwned($user, $autopilot);

        return response()->json([
            'data' => $this->autopilotPayload($entity),
        ]);
    }

    /**
     * Update one autopilot.
     */
    #[OpenApi\Operation(tags: ['Autopilot'])]
    #[OpenApi\RequestBody(factory: UpdateAutopilotRequestBody::class)]
    #[OpenApi\Response(factory: AutopilotResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function update(Request $request, string $autopilot, AutopilotService $autopilotService): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $validated = $request->validate([
            'displayName' => ['nullable', 'string', 'max:120'],
            'status' => ['nullable', 'string', 'in:active,disabled,archived'],
            'visibility' => ['nullable', 'string', 'in:private,discoverable,public'],
            'avatar' => ['nullable', 'string', 'max:255'],
            'tagline' => ['nullable', 'string', 'max:255'],

            'profile' => ['sometimes', 'array'],
            'profile.ownerDisplayName' => ['nullable', 'string', 'max:120'],
            'profile.personaSummary' => ['nullable', 'string'],
            'profile.autopilotVoice' => ['nullable', 'string', 'max:64'],
            'profile.principles' => ['nullable', 'array'],
            'profile.preferences' => ['nullable', 'array'],
            'profile.onboardingAnswers' => ['nullable', 'array'],
            'profile.schemaVersion' => ['nullable', 'integer', 'min:1'],

            'policy' => ['sometimes', 'array'],
            'policy.modelProvider' => ['nullable', 'string', 'max:64'],
            'policy.model' => ['nullable', 'string', 'max:128'],
            'policy.toolAllowlist' => ['nullable', 'array'],
            'policy.toolAllowlist.*' => ['string', 'max:128'],
            'policy.toolDenylist' => ['nullable', 'array'],
            'policy.toolDenylist.*' => ['string', 'max:128'],
            'policy.l402RequireApproval' => ['nullable', 'boolean'],
            'policy.l402MaxSpendMsatsPerCall' => ['nullable', 'integer', 'min:1'],
            'policy.l402MaxSpendMsatsPerDay' => ['nullable', 'integer', 'min:1'],
            'policy.l402AllowedHosts' => ['nullable', 'array'],
            'policy.l402AllowedHosts.*' => ['string', 'max:255'],
            'policy.dataPolicy' => ['nullable', 'array'],
        ]);

        $entity = $autopilotService->updateOwned($user, $autopilot, $validated);

        return response()->json([
            'data' => $this->autopilotPayload($entity),
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
    public function storeThread(
        Request $request,
        string $autopilot,
        AutopilotService $autopilotService,
        AutopilotThreadService $autopilotThreadService,
    ): JsonResponse {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $entity = $autopilotService->resolveOwned($user, $autopilot);

        $validated = $request->validate([
            'title' => ['nullable', 'string', 'max:200'],
        ]);

        $thread = $autopilotThreadService->ensureThread($user, $entity, null, (string) ($validated['title'] ?? ''));

        return response()->json([
            'data' => $this->threadPayload($thread),
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
    public function threads(Request $request, string $autopilot, AutopilotService $autopilotService): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $entity = $autopilotService->resolveOwned($user, $autopilot);

        $limit = max(1, min(200, (int) $request->integer('limit', 50)));

        $threads = Thread::query()
            ->where('user_id', $user->id)
            ->where('autopilot_id', $entity->id)
            ->orderByDesc('updated_at')
            ->limit($limit)
            ->get()
            ->map(fn (Thread $thread): array => $this->threadPayload($thread))
            ->values()
            ->all();

        return response()->json([
            'data' => $threads,
        ]);
    }

    /**
     * @return array<string, mixed>
     */
    private function autopilotPayload(Autopilot $autopilot): array
    {
        return [
            'id' => $autopilot->id,
            'handle' => $autopilot->handle,
            'displayName' => $autopilot->display_name,
            'status' => $autopilot->status,
            'visibility' => $autopilot->visibility,
            'ownerUserId' => (int) $autopilot->owner_user_id,
            'avatar' => $autopilot->avatar,
            'tagline' => $autopilot->tagline,
            'configVersion' => (int) $autopilot->config_version,
            'createdAt' => $autopilot->created_at?->toISOString(),
            'updatedAt' => $autopilot->updated_at?->toISOString(),
        ];
    }

    /**
     * @return array<string, mixed>
     */
    private function threadPayload(Thread $thread): array
    {
        return [
            'id' => (string) $thread->id,
            'autopilotId' => (string) $thread->autopilot_id,
            'title' => (string) $thread->title,
            'createdAt' => $thread->created_at?->toISOString(),
            'updatedAt' => $thread->updated_at?->toISOString(),
        ];
    }
}
