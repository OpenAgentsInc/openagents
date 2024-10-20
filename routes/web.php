<?php

use App\Http\Controllers\AuthController;
use App\Http\Controllers\ChatController;
use App\Http\Controllers\DashboardController;
use App\Http\Controllers\FreshController;
use App\Http\Controllers\MessageController;
use Illuminate\Support\Facades\Route;
use App\Http\Controllers\ThreadController;
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

Route::get('/threads', [ThreadController::class, 'index'])->name('threads.index'); // outside cuz the auth middleware is redirecting to login without the 401; fix?

Route::middleware(['auth'])->group(function () {

    Route::get('/fresh', [FreshController::class, 'fresh'])->name('fresh');
    Route::get('/chat/{thread}/messages', [FreshController::class, 'loadChatMessages'])->name('chat.messages');
    Route::post('/chat/{thread}/send', [FreshController::class, 'sendMessage'])->name('chat.send');

    Route::get('/chat', [ChatController::class, 'index'])->name('chat.index');
    Route::get('/chat/{thread}', [ChatController::class, 'show'])->name('chat.show');

    // For homepage - no thread yet
    Route::post('/send-message', [MessageController::class, 'sendMessage'])->name('send-message');

    // Thread routes
    Route::post('/threads/create', [ThreadController::class, 'create'])->name('threads.create');

    // Team routes
    Route::get('/teams-and-projects', [TeamController::class, 'getTeamsAndProjects'])->name('teams.projects');
    Route::post('/switch-team/{team}', [TeamController::class, 'switchTeam'])->name('switch-team');
    Route::post('/switch-project/{project}', [TeamController::class, 'switchProject'])->name('switch-project');
});

require __DIR__ . '/auth.php';