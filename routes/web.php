<?php

use App\Http\Controllers\AgentController;
use App\Http\Controllers\AuditController;
use App\Http\Controllers\AuthController;
use App\Http\Controllers\ConversationController;
use App\Http\Controllers\DashboardController;
use App\Http\Controllers\FileController;
use App\Http\Controllers\InspectController;
use App\Http\Controllers\MessageController;
use App\Http\Controllers\ProfileController;
use App\Http\Controllers\QueryController;
use App\Http\Controllers\StaticController;
use App\Http\Controllers\StatsController;
use App\Http\Controllers\StreamController;
use Illuminate\Support\Facades\Route;
use Inertia\Inertia;

Route::get('/', [StaticController::class, 'splash']);
Route::get('/terms', [StaticController::class, 'terms'])->name('terms');
Route::get('/privacy', [StaticController::class, 'privacy'])->name('privacy');
Route::get('/stats', [StatsController::class, 'index']);

Route::get('/login', [AuthController::class, 'login'])->name('login');
Route::get('/login/github', [AuthController::class, 'loginGithub']);
Route::get('/github', [AuthController::class, 'githubCallback']);

Route::get('/agent/{agent}', [AgentController::class, 'show'])->name('agent');

Route::get('/nodes', function () {
    return Inertia::render('Nodes');
});

Route::get('/chat', function () {
    return Inertia::render('Chat');
});

Route::post('/stream', [StreamController::class, 'chat']);

Route::group(['middleware' => ['auth']], function () {
    Route::get('/dashboard', [DashboardController::class, 'index'])->name('dashboard');
    Route::get('/referrals', [DashboardController::class, 'referrals'])->name('referrals');

    Route::post('/audit', [AuditController::class, 'store']);

    Route::get('/run/{id}', [InspectController::class, 'showRun'])->name('inspect-run');
    Route::get('/task/{id}', [InspectController::class, 'showTask'])->name('inspect-task');
    Route::get('/step/{id}', [InspectController::class, 'showStep'])->name('inspect-step');

    Route::any('/logout', [AuthController::class, 'logout'])->name('logout');
});

if (env('APP_ENV') !== "production") {
    Route::get('/inspect', [InspectController::class, 'index'])->name('inspect');

    Route::post('/api/agents', [AgentController::class, 'store'])
      ->middleware(['auth']);

    Route::post('/api/conversations', [ConversationController::class, 'store'])
      ->middleware(['auth'])
      ->name('conversations.store');

    Route::post('/api/messages', [MessageController::class, 'store'])
      ->middleware(['auth'])
      ->name('messages.store');

    Route::post('/api/files', [FileController::class, 'store'])
      ->name('files.store');

    Route::post('/api/query', [QueryController::class, 'store'])
      ->name('query.store');

    Route::post('/faerie-run', [AgentController::class, 'run'])
      ->middleware(['auth']);
}

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
