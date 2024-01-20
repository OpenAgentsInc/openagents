<?php

use App\Http\Controllers\AgentController;
use App\Http\Controllers\AuthController;
use App\Http\Controllers\BitcoinController;
use App\Http\Controllers\ConversationController;
use App\Http\Controllers\ExtismController;
use App\Http\Controllers\FileController;
use App\Http\Controllers\MessageController;
use App\Http\Controllers\PluginController;
use App\Http\Controllers\ProfileController;
use App\Http\Controllers\ReferralsController;
use App\Http\Controllers\StaticController;
use App\Http\Controllers\StatsController;
use App\Http\Controllers\StreamController;
use Illuminate\Support\Facades\Route;
use Inertia\Inertia;

// Route::get('/', [StaticController::class, 'splash']);
Route::get('/', [StaticController::class, 'newsplash']);

// Plugin uploading
Route::get('/plugins', [PluginController::class, 'index'])->name('plugins');
Route::post('/plugins', [PluginController::class, 'store']);

// Plugin test
Route::get('/extism', [ExtismController::class, 'test']);

// Bitcoin price
Route::get('/bitcoin', [BitcoinController::class, 'bitcoin'])->name('bitcoin');
Route::get('/bitcoin-price', [BitcoinController::class, 'bitcoinPrice'])->name('bitcoin-price');
Route::get('/bitcoin-price-ticker', [BitcoinController::class, 'sse'])->name('bitcoin-price-ticker');

// Concierge Chat
Route::get('/chat', [StreamController::class, 'chat'])->name('chat'); // OLD

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
    Route::get('/profile', [ProfileController::class, 'index'])->name('profile');
    Route::post('/update-profile', [ProfileController::class, 'update'])->name('update-profile');
    Route::get('/sse', [ProfileController::class, 'stream'])->name('stream');
    Route::get('/streamtest', [ProfileController::class, 'streamtest'])->name('streamtest');
    Route::get('/update-handler', [ProfileController::class, 'handleUpdate']);

    Route::get('/builder', [BuilderController::class, 'builder'])->name('build');
    Route::get('/referrals', [ReferralsController::class, 'referrals'])->name('referrals');
    Route::any('/logout', [AuthController::class, 'logout'])->name('logout');

    Route::post('/agents', [AgentController::class, 'store'])->name('agents.store');
    Route::post('/files', [FileController::class, 'store'])->name('files.store');
});

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
