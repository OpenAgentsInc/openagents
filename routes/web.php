<?php

use App\Http\Controllers\StaticController;
use App\Livewire\Splash;

Route::get('/', Splash::class)->name('home');

Route::get('/design', [StaticController::class, 'design'])->name('design');

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
