<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\ChatApiController;
use App\Http\Controllers\Controller;
use App\OpenApi\RequestBodies\ChatStreamRequestBody;
use App\OpenApi\Responses\NotFoundResponse;
use App\OpenApi\Responses\SseStreamResponse;
use App\OpenApi\Responses\UnauthorizedResponse;
use App\OpenApi\Responses\ValidationErrorResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;
use Laravel\Ai\Contracts\ConversationStore;
use Vyuldashev\LaravelOpenApi\Attributes as OpenApi;

#[OpenApi\PathItem]
class AutopilotStreamController extends Controller
{
    private const DEFAULT_AUTOPILOT_ID = 'default';

    /**
     * Stream a run via autopilot alias route.
     *
     * Phase A behavior: defaults to the built-in "default" autopilot and
     * forwards to the canonical chat stream runtime.
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
        ConversationStore $conversationStore,
    ) {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        if (strtolower(trim($autopilot)) !== self::DEFAULT_AUTOPILOT_ID) {
            abort(404);
        }

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

        if (! is_string($conversationId) || trim($conversationId) === '') {
            $conversationId = (string) $conversationStore->storeConversation($user->id, 'Autopilot conversation');

            $now = now();
            DB::table('threads')->insert([
                'id' => $conversationId,
                'user_id' => $user->id,
                'title' => 'Autopilot conversation',
                'created_at' => $now,
                'updated_at' => $now,
            ]);
        }

        $conversationId = trim((string) $conversationId);

        $conversationExists = DB::table('agent_conversations')
            ->where('id', $conversationId)
            ->where('user_id', $user->id)
            ->exists();

        if (! $conversationExists) {
            abort(404);
        }

        $request->query->set('conversationId', $conversationId);

        return $chatApiController->stream($request);
    }
}
