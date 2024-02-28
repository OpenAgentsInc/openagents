<?php

use App\Http\Controllers\Auth\NostrAuthController;
use App\Http\Controllers\AuthController;
use App\Http\Controllers\DocsController;
use App\Http\Controllers\StaticController;
use App\Livewire\AgentShow;
use App\Livewire\Chat;
use App\Livewire\CreatePassword;
use App\Livewire\Login;
use App\Livewire\Splash;
use Illuminate\Support\Facades\Route;

Route::get('/', Splash::class)->name('home');

// Agent chat
Route::get('/chat', Chat::class)->name('chat'); // todo - put behind auth middleware
Route::get('/chat/{id}', Chat::class)->name('chat.show');

// Agent view
Route::get('/agent/{id}', AgentShow::class)->name('agent.show');

// Docs
Route::get('/docs/{page}', [DocsController::class, 'show'])->name('docs.show');
Route::get('/docs/api/{apipage}', [DocsController::class, 'apidoc']);

// redirect /docs to /docs/introduction
Route::get('/docs', function () {
    return redirect('/docs/introduction');
});

// Dev only
Route::get('/design', [StaticController::class, 'design'])->name('design');
Route::get('/hud', [StaticController::class, 'hud'])->name('hud');

// Auth - nostr
Route::get('login/nostr', [NostrAuthController::class, 'client'])->name('loginnostrclient');
Route::post('login/nostr', [NostrAuthController::class, 'create'])->name('loginnostr');

// Auth - frontend
Route::get('/login', Login::class)->name('login');
Route::get('/create-password', CreatePassword::class)->name('create-password');

// Auth - backend
Route::any('/logout', [AuthController::class, 'logout']);

// Auth - social
Route::get('/login/github', [AuthController::class, 'loginGithub']);
Route::get('/github', [AuthController::class, 'githubCallback']);
Route::get('/login/twitter', [AuthController::class, 'loginTwitter']);
Route::get('/twitter', [AuthController::class, 'twitterCallback']);

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
