<?php

use Illuminate\Support\Facades\Route;
use App\Http\Controllers\Api\v1\DocumentsController;


Route::prefix('v1')->group(function () {
    Route::get('/agents/{agent}/documents',[DocumentsController::class,'index']);
});
