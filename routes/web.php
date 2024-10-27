<?php

use App\Http\Controllers\ChatController;
use App\Http\Controllers\ContentController;
use App\Http\Controllers\FileController;
use App\Http\Controllers\InquireController;
use App\Http\Controllers\ProfileController;
use App\Http\Controllers\UseChatController;
use Illuminate\Foundation\Application;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\Route;
use Inertia\Inertia;

// HOME
Route::get('/', function () {
    if (Auth::check()) {
        return redirect()->route('chat');
    }
    return Inertia::render('Welcome');
})->name('home');

Route::get('/new', function () {
    return Inertia::render('New');
});

// CHAT
Route::get('/chat', [ChatController::class, 'chat'])->name('chat');
Route::get('/chat/{id}', [ChatController::class, 'chat'])->name('chat.id');
Route::post('/chat', [UseChatController::class, 'chat']);

// CONTENT
Route::get('/thesis', [ContentController::class, 'thesis'])->name('content.thesis');

// INQUIRE
Route::get('/inquire', [InquireController::class, 'page'])->name('inquire');
Route::post('/inquire', [InquireController::class, 'submit'])->name('inquire.submit');

// FILES
Route::post('/api/files', [FileController::class, 'store'])
    // ->middleware('auth')
    ->name('files.store');

Route::get('/welcome', function () {
    return Inertia::render('Welcome', [
        'canLogin' => Route::has('login'),
        'canRegister' => Route::has('register'),
        'laravelVersion' => Application::VERSION,
        'phpVersion' => PHP_VERSION,
    ]);
});

Route::get('/dashboard', function () {
    return Inertia::render('Dashboard');
})->middleware(['auth', 'verified'])->name('dashboard');

Route::middleware('auth')->group(function () {
    Route::get('/profile', [ProfileController::class, 'edit'])->name('profile.edit');
    Route::patch('/profile', [ProfileController::class, 'update'])->name('profile.update');
    Route::delete('/profile', [ProfileController::class, 'destroy'])->name('profile.destroy');
});

require __DIR__ . '/components.php';
require __DIR__ . '/auth.php';
