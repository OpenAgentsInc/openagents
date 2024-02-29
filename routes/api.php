<?php

use App\Http\Controllers\API\AgentController;
use App\Http\Controllers\API\AgentFileController;
use App\Http\Controllers\API\FileController;
use Illuminate\Support\Facades\Route;

Route::middleware(['auth:sanctum'])->group(function () {
    Route::apiResource('agents', AgentController::class);
    Route::post('/agents/{agent}/files', [AgentFileController::class, 'store']);

    Route::apiResource('files', FileController::class);

    //    Route::apiResource('flows', FlowController::class);
    //    Route::apiResource('messages', MessageController::class);
    //    Route::apiResource('nodes', NodeController::class);
    //    Route::apiResource('plugins', PluginController::class);
    //    Route::apiResource('runs', RunController::class);
    //    Route::apiResource('threads', NodeController::class);
});
