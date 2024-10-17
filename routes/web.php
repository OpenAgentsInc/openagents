<?php

use Illuminate\Support\Facades\Route;
use App\Http\Controllers\MessageController;
use App\Http\Controllers\ThreadController;
use App\Http\Controllers\ProjectController;
use App\Http\Controllers\TeamController;

Route::get('/', function () {
    return auth()->check() ? view('dashboard') : view('homepage');
});

Route::view('/components', 'components')->name('components');

Route::middleware(['auth'])->group(function () {
    // Message routes
    Route::post('/messages', [MessageController::class, 'store']);
    Route::post('/threads/{thread}/messages', [MessageController::class, 'storeInThread']);

    // Thread routes
    Route::post('/threads/{thread}/process', [ThreadController::class, 'process']);

    // Project routes
    Route::get('/projects/{project}/threads', [ProjectController::class, 'threads']);

    // Team routes
    Route::get('/teams/{team}/threads', [TeamController::class, 'threads']);
});

require __DIR__ . '/auth.php';