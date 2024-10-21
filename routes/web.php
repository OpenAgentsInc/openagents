<?php

use App\Http\Controllers\AuthController;
use App\Http\Controllers\ChatController;
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
Route::view('/plans', function () {
    return "plans";
})->name('plans');

Route::middleware(['auth'])->group(function () {
    Route::get('/chat', [ChatController::class, 'index'])->name('chat.index');
    Route::get('/chat/{id}', [ChatController::class, 'show'])->name('chat.show');
    Route::post('/chat/{thread}/send', [ChatController::class, 'send'])->name('chat.send');


    // Message routes
    // Route::post('/messages', [MessageController::class, 'store']);
    // Route::post('/threads/{thread}/messages', [MessageController::class, 'storeInThread'])->name('messages.store');
    Route::post('/send-message', [MessageController::class, 'sendMessage'])->name('send-message');

    // // Thread routes
    Route::get('/threads', [ThreadController::class, 'index'])->name('threads.index');
    // Route::get('/threads/{thread}/messages', [ThreadController::class, 'messages'])->name('threads.messages');
    // Route::post('/threads/{thread}/process', [ThreadController::class, 'process']);
    // Route::get('/chat/{thread}', [ThreadController::class, 'show'])->name('chat.show');
    // Route::post('/threads/{thread}/add-message', [ThreadController::class, 'addMessage'])->name('threads.addMessage');
    Route::post('/threads/create', [ThreadController::class, 'create'])->name('threads.create');

    // // Project routes
    // Route::get('/projects/{project}/threads', [ProjectController::class, 'threads']);

    // // Team routes
    // Route::get('/teams/{team}/threads', [TeamController::class, 'threads']);
    Route::get('/teams', [TeamController::class, 'getTeamsAndProjects'])->name('teams.get');
    // Route::post('/switch-team/{team}', [TeamController::class, 'switchTeam'])->name('switch-team');
    // Route::post('/switch-project/{project}', [TeamController::class, 'switchProject'])->name('switch-project');

    // // New SSE route
    // Route::get('/chat/{thread}/stream', [MessageController::class, 'streamResponse'])->name('chat.stream');
});

require __DIR__ . '/auth.php';
