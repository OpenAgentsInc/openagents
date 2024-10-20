<?php

use Illuminate\Support\Facades\Route;
use App\Http\Controllers\TeamController;

Route::get('/', function () {
    return view('welcome');
});

Route::middleware(['auth:sanctum', 'verified'])->group(function () {
    Route::get('/dashboard', function () {
        return view('dashboard');
    })->name('dashboard');

    Route::get('/teams', [TeamController::class, 'getTeamsAndProjects'])->name('teams.get');
    Route::post('/switch-team/{team}', [TeamController::class, 'switchTeam'])->name('switch-team');
    Route::post('/switch-project/{project}', [TeamController::class, 'switchProject'])->name('switch-project');
});