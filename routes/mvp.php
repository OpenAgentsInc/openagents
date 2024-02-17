<?php

use App\Http\Controllers\StaticController;
use Illuminate\Support\Facades\Route;

// Homepage
Route::get('/', [StaticController::class, 'splash'])->name('home');

// Legal
Route::get('/terms', [StaticController::class, 'terms'])->name('terms');
Route::get('/privacy', [StaticController::class, 'privacy'])->name('privacy');

// Auth routes
require __DIR__.'/auth.php';

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
