<?php

use App\Http\Controllers\PrismController;
use App\Http\Controllers\StaticController;
use App\Livewire\Chat;
use App\Livewire\Frontpage;
use App\Livewire\ReverbDemo;
use Illuminate\Support\Facades\Route;

Route::get('/', Frontpage::class);
Route::get('/chat/{id}', Chat::class);
Route::get('/pro', [StaticController::class, 'pro']);
Route::get('/launch', [StaticController::class, 'launch']);
Route::get('/docs', [StaticController::class, 'docs']);
Route::get('/demo/reverb-chat', ReverbDemo::class);
Route::get('/nwc', [PrismController::class, 'nwc']);

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
