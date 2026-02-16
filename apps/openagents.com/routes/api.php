<?php

use App\Http\Controllers\Api\AdminController;
use App\Http\Controllers\Api\ChatController;
use App\Http\Controllers\Api\L402Controller;
use App\Http\Controllers\Api\MeController;
use App\Http\Controllers\Api\ProfileController;
use App\Http\Controllers\Api\TokenController;
use App\Http\Controllers\ChatApiController;
use Illuminate\Support\Facades\Route;

Route::middleware(['auth:sanctum'])->prefix('v1')->group(function () {
    Route::get('/me', [MeController::class, 'show']);

    Route::get('/admin/status', [AdminController::class, 'status'])
        ->middleware('admin');

    Route::get('/tokens', [TokenController::class, 'index']);
    Route::post('/tokens', [TokenController::class, 'store']);
    Route::delete('/tokens/current', [TokenController::class, 'destroyCurrent']);
    Route::delete('/tokens/{tokenId}', [TokenController::class, 'destroy'])
        ->whereNumber('tokenId');
    Route::delete('/tokens', [TokenController::class, 'destroyAll']);

    Route::get('/chats', [ChatController::class, 'index']);
    Route::post('/chats', [ChatController::class, 'store']);
    Route::get('/chats/{conversationId}', [ChatController::class, 'show']);
    Route::get('/chats/{conversationId}/messages', [ChatController::class, 'messages']);
    Route::get('/chats/{conversationId}/runs', [ChatController::class, 'runs']);
    Route::get('/chats/{conversationId}/runs/{runId}/events', [ChatController::class, 'runEvents']);
    Route::post('/chats/{conversationId}/stream', [ChatApiController::class, 'stream']);
    Route::post('/chat/stream', [ChatApiController::class, 'stream']);

    Route::get('/settings/profile', [ProfileController::class, 'show']);
    Route::patch('/settings/profile', [ProfileController::class, 'update']);
    Route::delete('/settings/profile', [ProfileController::class, 'destroy']);

    Route::prefix('/l402')->group(function () {
        Route::get('/wallet', [L402Controller::class, 'wallet']);
        Route::get('/transactions', [L402Controller::class, 'transactions']);
        Route::get('/transactions/{eventId}', [L402Controller::class, 'transactionShow'])
            ->whereNumber('eventId');
        Route::get('/paywalls', [L402Controller::class, 'paywalls']);
        Route::get('/settlements', [L402Controller::class, 'settlements']);
        Route::get('/deployments', [L402Controller::class, 'deployments']);
    });
});
