<?php

use App\Http\Controllers\AgentController;
use App\Http\Controllers\StaticController;
use App\Livewire\Chat;
use App\Livewire\Splash;

Route::get('/', Splash::class)->name('home');

// Agent chat
Route::get('/chat', Chat::class)->name('chat');
Route::post('/agent/{id}/run', [AgentController::class, 'run_task'])->name('agent.run_task');

// Dev only
Route::get('/design', [StaticController::class, 'design'])->name('design');

// Include breeze auth routes
require __DIR__.'/auth.php';

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
