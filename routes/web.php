<?php

use App\Http\Controllers\AgentController;
use App\Http\Controllers\AuthController;
use App\Http\Controllers\BuilderController;
use App\Http\Controllers\ConversationController;
use App\Http\Controllers\DashboardController;
use App\Http\Controllers\FileController;
use App\Http\Controllers\InspectController;
use App\Http\Controllers\MessageController;
use App\Http\Controllers\ProfileController;
use App\Http\Controllers\StaticController;
use App\Http\Controllers\StatsController;
use App\Http\Controllers\StreamController;
use Illuminate\Support\Facades\Route;
use Inertia\Inertia;

Route::get('/', [StaticController::class, 'splash']);

Route::get('/chat', function () {
    $streamer = new StreamController();
    $conversation = $streamer->fetchOrCreateConversation();
    return Inertia::render('Chat', [
        'conversationId' => $conversation->id,
    ]);
})->name('chat');

Route::get('/terms', [StaticController::class, 'terms'])->name('terms');
Route::get('/privacy', [StaticController::class, 'privacy'])->name('privacy');
Route::get('/stats', [StatsController::class, 'index']);

Route::get('/agents', [BuilderController::class, 'showcase'])->name('agents');
Route::get('/builder', [BuilderController::class, 'builder'])->name('build');

Route::get('/login', [AuthController::class, 'login'])->name('login');

Route::get('/login/github', [AuthController::class, 'loginGithub']);
Route::get('/github', [AuthController::class, 'githubCallback']);

Route::get('/login/twitter', [AuthController::class, 'loginTwitter']);
Route::get('/twitter', [AuthController::class, 'twitterCallback']);

Route::get('/agent/{id}', [AgentController::class, 'show'])->name('agent');
Route::post('/agent/{id}/chat', [AgentController::class, 'chat'])->name('agent.chat');

Route::post('/stream', [StreamController::class, 'chat']);

Route::group(['middleware' => ['auth']], function () {
    Route::get('/referrals', [DashboardController::class, 'referrals'])->name('referrals');
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

    Route::post('/faerie-run', [AgentController::class, 'run'])
      ->middleware(['auth']);
}

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
