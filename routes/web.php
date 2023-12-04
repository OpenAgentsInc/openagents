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

/*
|--------------------------------------------------------------------------
| Web Routes
|--------------------------------------------------------------------------
|
| Here is where you can register web routes for your application. These
| routes are loaded by the RouteServiceProvider within a group which
| contains the "web" middleware group. Now create something great!
|
*/

Route::get('/', function () {
    return Inertia::render('ComingSoon');
});

Route::get('/inspect', [InspectController::class, 'index'])->name('inspect');
Route::get('/inspect/{id}', [InspectController::class, 'show'])->name('inspect.show');

Route::get('/start', function () {
  return Inertia::render('Start');
})->name('start');

Route::get('/dashboard', function () {
    return Inertia::render('Dashboard');
})->middleware(['auth', 'verified'])->name('dashboard');

Route::middleware('auth')->group(function () {
    Route::get('/profile', [ProfileController::class, 'edit'])->name('profile.edit');
    Route::patch('/profile', [ProfileController::class, 'update'])->name('profile.update');
    Route::delete('/profile', [ProfileController::class, 'destroy'])->name('profile.destroy');
});

Route::post('/api/agents', [AgentController::class, 'store'])
  ->middleware(['auth']);

Route::post('/api/conversations', [ConversationController::class, 'store'])
  ->middleware(['auth'])
  ->name('conversations.store');

Route::post('/api/messages', [MessageController::class, 'store'])
  ->middleware(['auth'])
  ->name('messages.store');

Route::post('/api/files', [FileController::class, 'store'])
  // ->middleware('auth')
  ->name('files.store');

Route::post('/api/query', [QueryController::class, 'store'])
  ->name('query.store');

require __DIR__.'/auth.php';

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
  return redirect('/');
})->where('any', '.*');
