<?php

use App\Http\Controllers\AgentController;
use App\Http\Controllers\BitcoinController;
use App\Http\Controllers\PluginController;
use App\Http\Controllers\StaticController;
use Illuminate\Support\Facades\Route;

Route::get('/', [StaticController::class, 'newsplash'])->name('home');
Route::get('/blog', [StaticController::class, 'blog']);

Route::get('/withdraw', [BitcoinController::class, 'withdraw'])->name('withdraw');
Route::post('/withdraw', [BitcoinController::class, 'initiate_withdrawal'])->name('withdraw.initiate');

// Plugin uploading
Route::get('/plugins', [PluginController::class, 'index'])->name('plugins');
Route::get('/plugin/{plugin}', [PluginController::class, 'show'])->name('plugins.show');
Route::get('/plugins/create', [PluginController::class, 'create'])->name('plugins.create');
Route::post('/plugins', [PluginController::class, 'store'])->name('plugins.store');
Route::post('/plugins/call', [PluginController::class, 'call'])->name('plugins.call');

// Agents
Route::get('/agent/{id}', [AgentController::class, 'show'])->name('agent');
Route::post('/agent/{id}/run', [AgentController::class, 'run_task'])->name('agent.run_task');

// Static
Route::get('/terms', [StaticController::class, 'terms'])->name('terms');
Route::get('/privacy', [StaticController::class, 'privacy'])->name('privacy');

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
