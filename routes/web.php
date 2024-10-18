<?php

use App\Http\Controllers\AuthController;
use App\Http\Controllers\DashboardController;
use Illuminate\Support\Facades\Route;
use App\Http\Controllers\MessageController;
use App\Http\Controllers\ThreadController;
use App\Http\Controllers\ProjectController;
use App\Http\Controllers\TeamController;
use Illuminate\Support\Facades\Auth;

Route::get('/', [DashboardController::class, 'index'])->name('dashboard');

Route::get('/login', [AuthController::class, 'showLoginForm'])->name('login');
Route::get('/register', [AuthController::class, 'showRegistrationForm'])->name('register');
Route::get('/logout', function () {
    Auth::logout();
    request()->session()->invalidate();
    request()->session()->regenerateToken();
    return redirect('/');
});

Route::view('/components', 'components')->name('components');

Route::middleware(['auth'])->group(function () {
    // Message routes
    Route::post('/messages', [MessageController::class, 'store']);
    Route::post('/threads/{thread}/messages', [MessageController::class, 'storeInThread'])->name('messages.store');
    Route::post('/send-message', [MessageController::class, 'sendMessage'])->name('send-message');

    // Thread routes
    Route::post('/threads/{thread}/process', [ThreadController::class, 'process']);
    Route::get('/chat/{thread}', [ThreadController::class, 'show'])->name('chat.show');

    // Project routes
    Route::get('/projects/{project}/threads', [ProjectController::class, 'threads']);

    // Team routes
    Route::get('/teams/{team}/threads', [TeamController::class, 'threads']);

    // New SSE route
    Route::get('/chat/{thread}/stream', [MessageController::class, 'streamResponse'])->name('chat.stream');
});

require __DIR__ . '/auth.php';