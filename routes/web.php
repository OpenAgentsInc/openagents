<?php

use App\Http\Controllers\StaticController;
use App\Livewire\Splash;

Route::get('/', Splash::class)->name('home');

Route::get('/design', [StaticController::class, 'design'])->name('design');
