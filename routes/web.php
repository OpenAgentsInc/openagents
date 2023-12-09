<?php

use App\Http\Controllers\AgentController;
use App\Http\Controllers\ConversationController;
use App\Http\Controllers\FileController;
use App\Http\Controllers\InspectController;
use App\Http\Controllers\MessageController;
use App\Http\Controllers\ProfileController;
use App\Http\Controllers\QueryController;
use Illuminate\Foundation\Application;
use Illuminate\Support\Facades\Route;
use Inertia\Inertia;

Route::get('/', function () {
    return Inertia::render('Splash');
});

Route::get('/login', function () {
    return Inertia::render('Login');
});

Route::get('/inspect', [InspectController::class, 'index'])->name('inspect');
Route::get('/run/{id}', [InspectController::class, 'showRun'])->name('inspect-run');
Route::get('/task/{id}', [InspectController::class, 'showTask'])->name('inspect-task');
Route::get('/step/{id}', [InspectController::class, 'showStep'])->name('inspect-step');

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

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
  return redirect('/');
})->where('any', '.*');
