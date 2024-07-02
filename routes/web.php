<?php

use App\Http\Controllers\GreptileController;
use App\Http\Controllers\ProfileController;
use App\Http\Controllers\SSEController;
use Illuminate\Foundation\Application;
use Illuminate\Support\Facades\Route;
use Inertia\Inertia;

Route::get('/', function () {
    return Inertia::render('AutoDev');
    // return Inertia::render('Demo2');
})->name('home');

Route::post('/message', function () {
    return to_route('home')->with('success', 'Message sent successfully');
});

Route::post('/api/sse-stream', [SSEController::class, 'stream']);

Route::post('/api/index-repository', [GreptileController::class, 'indexRepository']);
Route::get('/api/repository-status/{repositoryId}', [GreptileController::class, 'getRepositoryStatus']);

Route::get('/dashboard', function () {
    return Inertia::render('Dashboard');
})->middleware(['auth', 'verified'])->name('dashboard');

Route::middleware('auth')->group(function () {
    Route::get('/profile', [ProfileController::class, 'edit'])->name('profile.edit');
    Route::patch('/profile', [ProfileController::class, 'update'])->name('profile.update');
    Route::delete('/profile', [ProfileController::class, 'destroy'])->name('profile.destroy');
});

require __DIR__.'/auth.php';
