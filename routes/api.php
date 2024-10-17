<?php

use Illuminate\Http\Request;
use Illuminate\Support\Facades\Route;
use App\Http\Controllers\MessageController;
use App\Http\Controllers\ThreadController;
use App\Http\Controllers\ProjectController;

Route::middleware(['auth:sanctum'])->group(function () {
    Route::get('/user', function (Request $request) {
        return $request->user();
    });

    // Message routes
    Route::post('/messages', [MessageController::class, 'store']);

    // Thread routes
    Route::post('/threads/{thread}/messages', [ThreadController::class, 'addMessage']);
    Route::post('/threads/{thread}/process', [ThreadController::class, 'process']);

    // Project routes
    Route::get('/projects/{project}/threads', [ProjectController::class, 'threads']);
});