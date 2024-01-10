<?php

use App\Http\Controllers\AgentController;
use App\Http\Controllers\AuthController;
use App\Http\Controllers\BuilderController;
use App\Http\Controllers\ConversationController;
use App\Http\Controllers\FileController;
use App\Http\Controllers\MessageController;
use App\Http\Controllers\ProfileController;
use App\Http\Controllers\ReferralsController;
use App\Http\Controllers\StaticController;
use App\Http\Controllers\StatsController;
use App\Http\Controllers\StreamController;
use Illuminate\Support\Facades\Route;
use Inertia\Inertia;

Route::get('/', [StaticController::class, 'splash']);

// Concierge Chat
Route::get('/chat', [StreamController::class, 'chat'])->name('chat');
Route::post('/stream', [StreamController::class, 'stream']);

// Agents
Route::get('/agent/{id}', [AgentController::class, 'show'])->name('agent');
Route::post('/agent/{id}/chat', [AgentController::class, 'chat'])->name('agent.chat');

// Static
Route::get('/terms', [StaticController::class, 'terms'])->name('terms');
Route::get('/privacy', [StaticController::class, 'privacy'])->name('privacy');
Route::get('/stats', [StatsController::class, 'index']);

// Auth
Route::get('/login', [AuthController::class, 'login'])->name('login');
Route::get('/login/github', [AuthController::class, 'loginGithub']);
Route::get('/github', [AuthController::class, 'githubCallback']);
Route::get('/login/twitter', [AuthController::class, 'loginTwitter']);
Route::get('/twitter', [AuthController::class, 'twitterCallback']);

// Authed routes
Route::group(['middleware' => ['auth']], function () {
    Route::get('/agents', [BuilderController::class, 'showcase'])->name('agents');
    Route::get('/builder', [BuilderController::class, 'builder'])->name('build');
    Route::get('/referrals', [ReferralsController::class, 'referrals'])->name('referrals');
    Route::any('/logout', [AuthController::class, 'logout'])->name('logout');

    Route::post('/agents', [AgentController::class, 'store'])->name('agents.store');
    Route::post('/api/files', [FileController::class, 'store'])->name('files.store');
});

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
