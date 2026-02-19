<?php

use App\Http\Controllers\Api\AgentPaymentsController;
use App\Http\Controllers\Api\AuthRegisterController;
use App\Http\Controllers\Api\AutopilotController;
use App\Http\Controllers\Api\AutopilotStreamController;
use App\Http\Controllers\Api\ChatController;
use App\Http\Controllers\Api\ConvexTokenController;
use App\Http\Controllers\Api\Internal\RuntimeSecretController;
use App\Http\Controllers\Api\L402Controller;
use App\Http\Controllers\Api\L402PaywallController;
use App\Http\Controllers\Api\MeController;
use App\Http\Controllers\Api\ProfileController;
use App\Http\Controllers\Api\RuntimeCodexWorkersController;
use App\Http\Controllers\Api\RuntimeSkillRegistryController;
use App\Http\Controllers\Api\RuntimeToolsController;
use App\Http\Controllers\Api\ShoutsController;
use App\Http\Controllers\Api\TokenController;
use App\Http\Controllers\Api\Webhooks\ResendWebhookController;
use App\Http\Controllers\Api\WhispersController;
use Illuminate\Support\Facades\Route;

// Public discoverability endpoints.
Route::get('/shouts', [ShoutsController::class, 'index']);
Route::get('/shouts/zones', [ShoutsController::class, 'zones']);

// Staging/automation bootstrap signup (disabled by default via config).
Route::post('/auth/register', [AuthRegisterController::class, 'store'])
    ->middleware('throttle:30,1');

Route::post('/webhooks/resend', [ResendWebhookController::class, 'store']);

$runtimeSecretFetchPath = ltrim((string) config('runtime.internal.secret_fetch_path', '/api/internal/runtime/integrations/secrets/fetch'), '/');
if (str_starts_with($runtimeSecretFetchPath, 'api/')) {
    $runtimeSecretFetchPath = substr($runtimeSecretFetchPath, 4);
}

Route::middleware('runtime.internal')
    ->post($runtimeSecretFetchPath, [RuntimeSecretController::class, 'fetch']);

Route::middleware(['auth:sanctum'])->group(function () {
    Route::get('/me', [MeController::class, 'show']);

    Route::get('/tokens', [TokenController::class, 'index']);
    Route::post('/tokens', [TokenController::class, 'store']);
    Route::delete('/tokens/current', [TokenController::class, 'destroyCurrent']);
    Route::delete('/tokens/{tokenId}', [TokenController::class, 'destroy'])
        ->whereNumber('tokenId');
    Route::delete('/tokens', [TokenController::class, 'destroyAll']);
    Route::post('/convex/token', [ConvexTokenController::class, 'store']);

    Route::get('/chats', [ChatController::class, 'index']);
    Route::post('/chats', [ChatController::class, 'store']);
    Route::get('/chats/{conversationId}', [ChatController::class, 'show']);
    Route::get('/chats/{conversationId}/messages', [ChatController::class, 'messages']);
    Route::get('/chats/{conversationId}/runs', [ChatController::class, 'runs']);
    Route::get('/chats/{conversationId}/runs/{runId}/events', [ChatController::class, 'runEvents']);
    Route::post('/chats/{conversationId}/stream', [ChatController::class, 'stream']);
    Route::post('/chat/stream', [ChatController::class, 'stream']);
    Route::post('/runtime/tools/execute', [RuntimeToolsController::class, 'execute']);
    Route::get('/runtime/skills/tool-specs', [RuntimeSkillRegistryController::class, 'listToolSpecs']);
    Route::post('/runtime/skills/tool-specs', [RuntimeSkillRegistryController::class, 'storeToolSpec']);
    Route::get('/runtime/skills/skill-specs', [RuntimeSkillRegistryController::class, 'listSkillSpecs']);
    Route::post('/runtime/skills/skill-specs', [RuntimeSkillRegistryController::class, 'storeSkillSpec']);
    Route::post('/runtime/skills/skill-specs/{skillId}/{version}/publish', [RuntimeSkillRegistryController::class, 'publishSkillSpec']);
    Route::get('/runtime/skills/releases/{skillId}/{version}', [RuntimeSkillRegistryController::class, 'showSkillRelease']);
    Route::get('/runtime/codex/workers', [RuntimeCodexWorkersController::class, 'index']);
    Route::post('/runtime/codex/workers', [RuntimeCodexWorkersController::class, 'create']);
    Route::get('/runtime/codex/workers/{workerId}', [RuntimeCodexWorkersController::class, 'show']);
    Route::get('/runtime/codex/workers/{workerId}/stream', [RuntimeCodexWorkersController::class, 'stream']);
    Route::post('/runtime/codex/workers/{workerId}/requests', [RuntimeCodexWorkersController::class, 'request']);
    Route::post('/runtime/codex/workers/{workerId}/events', [RuntimeCodexWorkersController::class, 'events']);
    Route::post('/runtime/codex/workers/{workerId}/stop', [RuntimeCodexWorkersController::class, 'stop']);

    // Autopilot API (phase A skeleton).
    Route::get('/autopilots', [AutopilotController::class, 'index']);
    Route::post('/autopilots', [AutopilotController::class, 'store']);
    Route::get('/autopilots/{autopilot}', [AutopilotController::class, 'show']);
    Route::patch('/autopilots/{autopilot}', [AutopilotController::class, 'update']);
    Route::get('/autopilots/{autopilot}/threads', [AutopilotController::class, 'threads']);
    Route::post('/autopilots/{autopilot}/threads', [AutopilotController::class, 'storeThread']);
    Route::post('/autopilots/{autopilot}/stream', [AutopilotStreamController::class, 'stream']);

    Route::get('/settings/profile', [ProfileController::class, 'show']);
    Route::patch('/settings/profile', [ProfileController::class, 'update']);
    Route::delete('/settings/profile', [ProfileController::class, 'destroy']);

    Route::post('/shouts', [ShoutsController::class, 'store']);

    Route::get('/whispers', [WhispersController::class, 'index']);
    Route::post('/whispers', [WhispersController::class, 'store']);
    Route::patch('/whispers/{id}/read', [WhispersController::class, 'read'])->whereNumber('id');

    // Agent Payments API (Laravel port of Episode 169-style wallet endpoints)
    Route::prefix('/agent-payments')->group(function () {
        Route::get('/wallet', [AgentPaymentsController::class, 'wallet']);
        Route::post('/wallet', [AgentPaymentsController::class, 'upsertWallet']);
        Route::get('/balance', [AgentPaymentsController::class, 'balance']);
        Route::post('/invoice', [AgentPaymentsController::class, 'createInvoice']);
        Route::post('/pay', [AgentPaymentsController::class, 'payInvoice']);
        Route::post('/send-spark', [AgentPaymentsController::class, 'sendSpark']);
    });

    // Backward-compatible aliases for older OpenAgents Agent Payments API shape.
    Route::get('/agents/me/wallet', [AgentPaymentsController::class, 'wallet']);
    Route::post('/agents/me/wallet', [AgentPaymentsController::class, 'upsertWallet']);
    Route::get('/agents/me/balance', [AgentPaymentsController::class, 'balance']);
    Route::post('/payments/invoice', [AgentPaymentsController::class, 'createInvoice']);
    Route::post('/payments/pay', [AgentPaymentsController::class, 'payInvoice']);
    Route::post('/payments/send-spark', [AgentPaymentsController::class, 'sendSpark']);

    Route::prefix('/l402')->group(function () {
        Route::get('/wallet', [L402Controller::class, 'wallet']);
        Route::get('/transactions', [L402Controller::class, 'transactions']);
        Route::get('/transactions/{eventId}', [L402Controller::class, 'transactionShow'])
            ->whereNumber('eventId');
        Route::get('/paywalls', [L402Controller::class, 'paywalls']);
        Route::post('/paywalls', [L402PaywallController::class, 'store'])->middleware('admin');
        Route::patch('/paywalls/{paywallId}', [L402PaywallController::class, 'update'])->middleware('admin');
        Route::delete('/paywalls/{paywallId}', [L402PaywallController::class, 'destroy'])->middleware('admin');
        Route::get('/settlements', [L402Controller::class, 'settlements']);
        Route::get('/deployments', [L402Controller::class, 'deployments']);
    });
});
