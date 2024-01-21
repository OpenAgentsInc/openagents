<?php

use App\Http\Controllers\PluginController;
use App\Http\Controllers\StaticController;
use Illuminate\Support\Facades\Route;

Route::get('/', [StaticController::class, 'newsplash'])->name('home');
Route::get('/blog', [StaticController::class, 'blog']);

// Plugin uploading
Route::get('/plugins', [PluginController::class, 'index'])->name('plugins');
Route::get('/plugin/{plugin}', [PluginController::class, 'show'])->name('plugins.show');
Route::get('/plugins/create', [PluginController::class, 'create'])->name('plugins.create');
Route::post('/plugins', [PluginController::class, 'store'])->name('plugins.store');

// Static
Route::get('/terms', [StaticController::class, 'terms'])->name('terms');
Route::get('/privacy', [StaticController::class, 'privacy'])->name('privacy');

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
