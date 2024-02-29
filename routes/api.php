<?php

use App\Http\Controllers\API\AgentController;
use App\Http\Controllers\API\AgentFileController;
use Illuminate\Support\Facades\Route;

Route::middleware(['auth:sanctum'])->group(function () {
    // Create an agent
    Route::post('/agents', [AgentController::class, 'store'])->name('api.agents.store');

    // Retrieve a list of all agents
    Route::get('/agents', [AgentController::class, 'index'])->name('api.agents.index');

    // Retrieve a specific agent
    Route::get('/agents/{agent}', [AgentController::class, 'show'])->name('api.agents.show');

    // Update a specific agent
    Route::put('/agents/{agent}', [AgentController::class, 'update'])->name('api.agents.update');

    // Delete a specific agent
    Route::delete('/agents/{agent}', [AgentController::class, 'destroy'])->name('api.agents.destroy');

    // Add file to an agent
    Route::post('/agents/{agent}/files', [AgentFileController::class, 'store'])->name('api.agents.files.store');

    //    Route::apiResource('agents', AgentController::class);
    //    Route::apiResource('files', FileController::class);
    //    Route::apiResource('flows', FlowController::class);
    //    Route::apiResource('messages', MessageController::class);
    //    Route::apiResource('nodes', NodeController::class);
    //    Route::apiResource('plugins', PluginController::class);
    //    Route::apiResource('runs', RunController::class);
    //    Route::apiResource('threads', NodeController::class);
});
