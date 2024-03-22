<?php

use App\Livewire\Chat;
use App\Livewire\Frontpage;
use Illuminate\Support\Facades\Route;
use App\Http\Controllers\StaticController;

Route::get('/', Frontpage::class);
Route::get('/chat/{id}', Chat::class);
Route::get('/launch', [StaticController::class, 'launch']);
Route::get('/docs', [StaticController::class, 'docs']);
Route::get('/demo/reverb-chat', App\Livewire\ReverbDemo::class);

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
