<?php

use App\Http\Controllers\ChatApiController;
use App\Http\Controllers\ChatPageController;
use App\Http\Controllers\FeedPageController;
use App\Http\Controllers\GuestChatSessionController;
use App\Http\Controllers\L402PageController;
use App\Http\Controllers\OpenApiSpecController;
use App\Http\Middleware\ValidateWorkOSSession;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Route;
use Inertia\Inertia;

Route::get('/', fn () => Inertia::render('index'))->name('home');

Route::get('api/chat/guest-session', GuestChatSessionController::class)->name('api.chat.guest-session');

Route::post('api/chat', [ChatApiController::class, 'stream'])
    ->middleware(ValidateWorkOSSession::class)
    ->name('api.chat');

Route::get('chat/{conversationId?}', [ChatPageController::class, 'show'])
    ->middleware(ValidateWorkOSSession::class)
    ->name('chat');

Route::get('openapi.json', [OpenApiSpecController::class, 'show'])->name('openapi.default.specification');

// Lightweight SSE smoke endpoint (auth-less) for infra validation.
// Gate with a header secret to avoid exposing it publicly.
Route::get('api/smoke/stream', function (Request $request) {
    $secret = (string) env('OA_SMOKE_SECRET', '');
    $provided = (string) $request->header('x-oa-smoke-secret', '');

    if ($secret === '' || ! hash_equals($secret, $provided)) {
        abort(401);
    }

    return response()->stream(function () {
        $write = function (array $payload): void {
            echo 'data: ' . json_encode($payload) . "\n\n";

            if (ob_get_level() > 0) {
                ob_flush();
            }
            flush();
        };

        $write(['type' => 'start', 'id' => 'smoke']);

        $parts = ['hello', ' ', 'from', ' ', 'smoke', ' ', 'stream'];
        foreach ($parts as $delta) {
            $write(['type' => 'text-delta', 'delta' => $delta]);
            usleep(150000);
        }

        $write(['type' => 'finish', 'finishReason' => 'stop']);

        echo "data: [DONE]\n\n";
        if (ob_get_level() > 0) {
            ob_flush();
        }
        flush();
    }, 200, [
        'Cache-Control' => 'no-cache, no-transform',
        'Content-Type' => 'text/event-stream',
        'x-vercel-ai-ui-message-stream' => 'v1',
        'x-oa-smoke' => '1',
    ]);
})->name('api.smoke.stream');

Route::get('aui', fn() => Inertia::render('aui'))->name('aui');

Route::middleware([
    'auth',
    ValidateWorkOSSession::class,
])->group(function () {
    Route::get('feed', [FeedPageController::class, 'index'])->name('feed');

    Route::prefix('l402')->name('l402.')->group(function () {
        Route::get('/', [L402PageController::class, 'wallet'])->name('wallet');
        Route::get('/transactions', [L402PageController::class, 'transactions'])->name('transactions.index');
        Route::get('/transactions/{eventId}', [L402PageController::class, 'transactionShow'])
            ->whereNumber('eventId')
            ->name('transactions.show');
        Route::get('/paywalls', [L402PageController::class, 'paywalls'])->name('paywalls');
        Route::get('/settlements', [L402PageController::class, 'settlements'])->name('settlements');
        Route::get('/deployments', [L402PageController::class, 'deployments'])->name('deployments');
    });

    Route::get('admin', function () {
        return Inertia::render('admin/index');
    })->middleware('admin')->name('admin');
});

require __DIR__ . '/settings.php';
require __DIR__ . '/auth.php';
