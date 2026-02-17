<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\ChatApiController;
use App\Http\Controllers\Controller;
use App\OpenApi\RequestBodies\ChatStreamRequestBody;
use App\OpenApi\Responses\NotFoundResponse;
use App\OpenApi\Responses\SseStreamResponse;
use App\OpenApi\Responses\UnauthorizedResponse;
use App\OpenApi\Responses\ValidationErrorResponse;
use App\Services\AutopilotService;
use App\Services\AutopilotThreadService;
use Illuminate\Http\Request;
use Vyuldashev\LaravelOpenApi\Attributes as OpenApi;

#[OpenApi\PathItem]
class AutopilotStreamController extends Controller
{
    /**
     * Stream a run via autopilot alias route.
     *
     * Resolves the owned autopilot, ensures a compatible thread exists,
     * and then forwards to the canonical chat stream runtime.
     */
    #[OpenApi\Operation(tags: ['Autopilot'])]
    #[OpenApi\RequestBody(factory: ChatStreamRequestBody::class)]
    #[OpenApi\Response(factory: SseStreamResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function stream(
        Request $request,
        string $autopilot,
        ChatApiController $chatApiController,
        AutopilotService $autopilotService,
        AutopilotThreadService $autopilotThreadService,
    ) {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $entity = $autopilotService->resolveOwned($user, $autopilot);

        $conversationId = $request->route('conversationId');
        if (! is_string($conversationId) || trim($conversationId) === '') {
            $conversationId = $request->query('conversationId');
        }
        if (! is_string($conversationId) || trim($conversationId) === '') {
            $conversationId = $request->input('conversationId');
        }
        if (! is_string($conversationId) || trim($conversationId) === '') {
            $conversationId = $request->input('threadId');
        }

        $thread = $autopilotThreadService->ensureThread(
            $user,
            $entity,
            is_string($conversationId) ? $conversationId : null,
            'Autopilot conversation',
        );

        $request->query->set('conversationId', $thread->id);
        if ($request->route()) {
            $request->route()->setParameter('conversationId', $thread->id);
        }

        return $chatApiController->stream($request);
    }
}
