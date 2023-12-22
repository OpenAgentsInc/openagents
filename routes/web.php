<?php

use App\Http\Controllers\AgentController;
use App\Http\Controllers\AuditController;
use App\Http\Controllers\ConversationController;
use App\Http\Controllers\DashboardController;
use App\Http\Controllers\FileController;
use App\Http\Controllers\InspectController;
use App\Http\Controllers\MessageController;
use App\Http\Controllers\ProfileController;
use App\Http\Controllers\QueryController;
use App\Models\User;
use App\Services\Auditor;
use Illuminate\Foundation\Application;
use Illuminate\Support\Facades\Route;
use Inertia\Inertia;
use Laravel\Socialite\Facades\Socialite;

Route::get('/', function () {
    return Inertia::render('Splash');
});

Route::get('/chat', function () {
    return Inertia::render('Chat');
});

Route::post('/audit', [AuditController::class, 'store']);

Route::get('/dashboard', [DashboardController::class, 'index'])
    ->middleware(['auth'])
    ->name('dashboard');

Route::get('/login', function () {
    return Inertia::render('Login');
})->name('login');

Route::get('/login/github', function () {
    return Socialite::driver('github')->redirect();
});

Route::get('/github', function () {
    $githubUser = Socialite::driver('github')->user();

    $user = User::updateOrCreate(
        ['github_id' => $githubUser->id], // Check if GitHub ID exists
        [
            'name' => $githubUser->name,
            'email' => $githubUser->email,
            'github_nickname' => $githubUser->nickname,
            'github_avatar' => $githubUser->avatar,
        ]
    );

    // Log in this user
    auth()->login($user, true);

    return redirect('/dashboard');
});

Route::get('/run/{id}', [InspectController::class, 'showRun'])->name('inspect-run');
Route::get('/task/{id}', [InspectController::class, 'showTask'])->name('inspect-task');
Route::get('/step/{id}', [InspectController::class, 'showStep'])->name('inspect-step');

Route::get('/terms', function () {
    return Inertia::render('Terms');
})->name('terms');

Route::get('/privacy', function () {
    return Inertia::render('Privacy');
})->name('privacy');

Route::any('/logout', function () {
    auth()->logout();
    return redirect('/');
})->name('logout');

if (env('APP_ENV') !== "production") {
    Route::get('/inspect', [InspectController::class, 'index'])->name('inspect');

    Route::post('/api/agents', [AgentController::class, 'store'])
      ->middleware(['auth']);

    Route::post('/api/conversations', [ConversationController::class, 'store'])
      ->middleware(['auth'])
      ->name('conversations.store');

    Route::post('/api/messages', [MessageController::class, 'store'])
      ->middleware(['auth'])
      ->name('messages.store');

    Route::post('/api/files', [FileController::class, 'store'])
      ->name('files.store');

    Route::post('/api/query', [QueryController::class, 'store'])
      ->name('query.store');

    Route::post('/faerie-run', [AgentController::class, 'run'])
      ->middleware(['auth']);
}

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
