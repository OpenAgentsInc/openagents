<?php

use App\Http\Controllers\AgentController;
use App\Http\Controllers\AuthController;
use App\Http\Controllers\BitcoinController;
use App\Http\Controllers\ProfileController;
use App\Http\Controllers\PluginController;
use App\Http\Controllers\StaticController;
use App\Livewire\Chat;
use App\Livewire\PluginList;
use Illuminate\Support\Facades\Route;

Route::get('/', [StaticController::class, 'splash'])->name('home');

// Disable all these routes in production
if (!app()->environment('production')) {
    Route::get('/design', [StaticController::class, 'design'])->name('design');

    Route::get('/agent/chat', Chat::class)->name('agent.chat');

    // Dashboard placeholder
    Route::get('/dashboard', function () {
        return view('dashboard');
    })->middleware(['auth', 'verified'])->name('dashboard');

    Route::get('/earnings', function () {
        return view('earnings');
    });

    Route::middleware('auth')->group(function () {
        Route::get('/profile', [ProfileController::class, 'edit'])->name('profile.edit');
        Route::patch('/profile', [ProfileController::class, 'update'])->name('profile.update');
        Route::delete('/profile', [ProfileController::class, 'destroy'])->name('profile.destroy');
    });

    // Plugin uploading
    Route::get('/plugins', PluginList::class)->name('plugins.index');
    Route::get('/plugin/{plugin}', [PluginController::class, 'show'])->name('plugins.show');
    Route::get('/plugins/create', [PluginController::class, 'create'])->name('plugins.create');
    Route::post('/plugins', [PluginController::class, 'store'])->name('plugins.store');
    Route::post('/plugins/call', [PluginController::class, 'call'])->name('plugins.call');

    // Agents (public)
    Route::get('/agents', [AgentController::class, 'index'])->name('agents.index');
    Route::get('/agent/connie', [AgentController::class, 'coder'])->name('agent.coder');
    Route::get('/agent/{id}', [AgentController::class, 'show'])->name('agent');

    // Auth
    Route::get('/login', [AuthController::class, 'login']);
    Route::get('/login/github', [AuthController::class, 'loginGithub']);
    Route::get('/github', [AuthController::class, 'githubCallback']);
    Route::get('/login/twitter', [AuthController::class, 'loginTwitter']);
    Route::get('/twitter', [AuthController::class, 'twitterCallback']);

    // Authed routes
    Route::middleware(['auth'])->group(function () {
        // Agents (authed)
        Route::get('/agents/create', [AgentController::class, 'create'])->name('agents.create');
        Route::post('/agents', [AgentController::class, 'store'])->name('agents.store');
        Route::post('/agent/{id}/run', [AgentController::class, 'run_task'])->name('agent.run_task');

        // Agent builder
        Route::get('/agent/{id}/build', [AgentController::class, 'build'])->name('agent.build');

        // Withdrawals
        Route::get('/withdraw', [BitcoinController::class, 'withdraw'])->name('withdraw');
        Route::post('/withdraw', [BitcoinController::class, 'initiate_withdrawal'])->name('withdraw.initiate');
    });

    require __DIR__.'/auth.php';
}

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
