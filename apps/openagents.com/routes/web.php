<?php

use App\Http\Controllers\ChatApiController;
use App\Http\Controllers\ChatPageController;
use Illuminate\Support\Facades\Route;
use Inertia\Inertia;
use Laravel\WorkOS\Http\Middleware\ValidateSessionWithWorkOS;

Route::get('/', fn () => Inertia::render('welcome'))->name('home');

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
