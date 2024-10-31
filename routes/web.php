<?php

use App\Http\Controllers\ChatController;
use App\Http\Controllers\ContentController;
use App\Http\Controllers\CRMController;
use App\Http\Controllers\FileController;
use App\Http\Controllers\InquireController;
use App\Http\Controllers\ProfileController;
use App\Http\Controllers\ProjectController;
use App\Http\Controllers\TeamController;
use App\Http\Controllers\UseChatController;
use Illuminate\Foundation\Application;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\Route;
use Inertia\Inertia;

// HOME
Route::get('/', function () {
    if (Auth::check()) {
        return redirect()->route('chat');
    }
    return Inertia::render('New');
})->name('home');


Route::middleware('auth')->group(function () {
    // CHAT
    Route::get('/chat', [ChatController::class, 'index'])->name('chat');
    Route::any('/chat/create', [ChatController::class, 'create'])->name('chat.create');
    Route::get('/chat/{id}', [ChatController::class, 'show'])->name('chat.id');
    Route::post('/chat', [UseChatController::class, 'chat']);
    Route::delete('/chat/{id}', [ChatController::class, 'destroy'])->name('chat.destroy');

    // CRM
    Route::get('/crm', [CRMController::class, 'index'])->name('crm');

    // TEAMS
    Route::get('/teams/create', [TeamController::class, 'create'])->name('teams.create');
    Route::post('/teams', [TeamController::class, 'store'])->name('teams.store');
    Route::post('/switch-team', [TeamController::class, 'switchTeam'])->name('teams.switch');

    // PROJECTS
    Route::get('/projects/create', [ProjectController::class, 'create'])->name('projects.create');
    Route::post('/projects', [ProjectController::class, 'store'])->name('projects.store');
});

// CONTENT
Route::get('/thesis', [ContentController::class, 'thesis'])->name('content.thesis');
Route::get('/terms', [ContentController::class, 'terms'])->name('content.terms');
Route::get('/privacy', [ContentController::class, 'privacy'])->name('content.privacy');

// INQUIRE
Route::get('/inquire', [InquireController::class, 'page'])->name('inquire');
Route::post('/inquire', [InquireController::class, 'submit'])->name('inquire.submit');

// FILES
Route::post('/api/files', [FileController::class, 'store'])
    // ->middleware('auth')
    ->name('files.store');

Route::get('/welcome', function () {
    return Inertia::render('Welcome', [
        'canLogin' => Route::has('login'),
        'canRegister' => Route::has('register'),
        'laravelVersion' => Application::VERSION,
        'phpVersion' => PHP_VERSION,
    ]);
});

Route::get('/dashboard', function () {
    // return redirect to home lol
    return redirect('/');
    // return Inertia::render('Dashboard');
})->middleware(['auth', 'verified'])->name('dashboard');

Route::middleware('auth')->group(function () {
    Route::get('/profile', [ProfileController::class, 'edit'])->name('profile.edit');
    Route::patch('/profile', [ProfileController::class, 'update'])->name('profile.update');
    Route::delete('/profile', [ProfileController::class, 'destroy'])->name('profile.destroy');
});

require __DIR__ . '/components.php';
require __DIR__ . '/auth.php';

// Catchall redirect to /
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
