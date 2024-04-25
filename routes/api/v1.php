<?php

use App\Http\Controllers\Api\v1\DocumentsController;
use Illuminate\Support\Facades\Route;

Route::prefix('v1')->group(function () {
    Route::get('/agents/{agent}/documents', [DocumentsController::class, 'index']);
});
