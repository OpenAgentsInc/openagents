<?php

use App\Http\Controllers\AuthController;
use App\Http\Controllers\DocsController;
use App\Http\Controllers\StaticController;
use App\Livewire\AgentShow;
use App\Livewire\Chat;
use App\Livewire\CreatePassword;
use App\Livewire\Frontpage;
use App\Livewire\Login;
use App\Livewire\PayBitcoin;
use Illuminate\Support\Facades\Route;

Route::get('/', Frontpage::class)->name('home');

// Billing
Route::get('/pay/bitcoin', PayBitcoin::class);

// Agent chat
Route::get('/chat', Chat::class)->name('chat'); // todo - put behind auth middleware
Route::get('/chat/{id}', Chat::class)->name('chat.show');

// Agent view
Route::get('/agent/{id}', AgentShow::class)->name('agent.show');

// Docs
Route::get('/docs/{page}', [DocsController::class, 'show'])->name('docs.show');
Route::get('/docs/api/{path?}', [DocsController::class, 'new'])
    ->where('path', '.*'); // Allows "path" parameter to include slashes

// redirect /docs to /docs/introduction
Route::get('/docs', function () {
    return redirect('/docs/introduction');
});

// Dev only
Route::get('/design', [StaticController::class, 'design'])->name('design');
Route::get('/hud', [StaticController::class, 'hud'])->name('hud');

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
