<?php

use App\Livewire\Chat;
use App\Livewire\Frontpage;
use Illuminate\Support\Facades\Route;

Route::get('/', Frontpage::class);
Route::get('/chat/{id}', Chat::class);

//Route::middleware([
//    'auth:sanctum',
//    config('jetstream.auth_session'),
//    'verified',
//])->group(function () {
//    Route::get('/dashboard', function () {
//        return view('dashboard');
//    })->name('dashboard');
//});

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
