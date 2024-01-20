<?php

use App\Http\Controllers\PluginController;
use App\Http\Controllers\StaticController;
use Illuminate\Support\Facades\Route;

Route::get('/', [StaticController::class, 'newsplash']);
Route::get('/accelerate', [StaticController::class, 'accelerate']);

// Plugin uploading
Route::get('/plugins', [PluginController::class, 'index'])->name('plugins');
Route::post('/plugins', [PluginController::class, 'store']);

// Static
Route::get('/terms', [StaticController::class, 'terms'])->name('terms');
Route::get('/privacy', [StaticController::class, 'privacy'])->name('privacy');

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
