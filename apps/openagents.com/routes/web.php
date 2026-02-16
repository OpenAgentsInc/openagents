<?php

use App\Http\Controllers\ChatApiController;
use App\Http\Controllers\ChatPageController;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Route;
use Inertia\Inertia;
use Laravel\WorkOS\Http\Middleware\ValidateSessionWithWorkOS;

Route::get('/', fn () => Inertia::render('welcome'))->name('home');

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
            echo 'data: '.json_encode($payload).'

';

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

        echo 'data: [DONE]

';
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

Route::middleware([
    'auth',
    ValidateSessionWithWorkOS::class,
])->group(function () {
    Route::get('dashboard', function () {
        return Inertia::render('dashboard');
    })->name('dashboard');

    Route::get('chat/{conversationId?}', [ChatPageController::class, 'show'])
        ->name('chat');

    // Vercel AI SDK-compatible endpoint (SSE, Vercel data stream protocol).
    Route::post('api/chat', [ChatApiController::class, 'stream'])
        ->name('api.chat');
});

require __DIR__.'/settings.php';
require __DIR__.'/auth.php';
