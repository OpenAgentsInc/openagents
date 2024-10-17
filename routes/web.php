<?php

use Illuminate\Support\Facades\Route;
use App\Http\Controllers\MessageController;
use App\Http\Controllers\ThreadController;
use App\Http\Controllers\ProjectController;
use App\Http\Controllers\TeamController;
use App\Http\Controllers\AuthController;

Route::get('/', function () {
    return auth()->check() ? view('dashboard') : view('homepage');
});

Route::view('/components', 'components')->name('components');

// Custom Authentication Routes
Route::get('/login', [AuthController::class, 'showLoginForm'])->name('login');
Route::get('/register', [AuthController::class, 'showRegistrationForm'])->name('register');

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