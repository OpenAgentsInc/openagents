<?php

use App\Http\Controllers\AgentController;
use App\Http\Controllers\AuthController;
use App\Http\Controllers\StaticController;
use App\Livewire\Chat;
use App\Livewire\Splash;

Route::get('/', Splash::class)->name('home');

// Agent chat
Route::get('/chat', Chat::class)->name('chat'); // todo - put behind auth middleware
Route::post('/agent/{id}/run', [AgentController::class, 'run_task'])->name('agent.run_task');

// Dev only
Route::get('/design', [StaticController::class, 'design'])->name('design');
Route::get('/hud', [StaticController::class, 'hud'])->name('hud');

// Include breeze auth routes
require __DIR__.'/auth.php';

Route::get('/login/github', [AuthController::class, 'loginGithub']);
Route::get('/github', [AuthController::class, 'githubCallback']);
Route::get('/login/twitter', [AuthController::class, 'loginTwitter']);
Route::get('/twitter', [AuthController::class, 'twitterCallback']);

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
