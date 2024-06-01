<?php

use App\Http\Controllers\Api\v1\DocumentsController;
use App\Http\Controllers\Api\v1\PluginsController;
use Illuminate\Support\Facades\Route;

Route::prefix('v1')->group(function () {
    Route::get('/agents/{agent}/documents', [DocumentsController::class, 'index']);
    Route::get('/plugins', [PluginsController::class, 'index']);
    Route::get('/plugins/view/{plugin}', [PluginsController::class, 'show'])->name('api.plugins.view');
    Route::get('/plugins/secrets', [PluginsController::class, 'secret']);
});
