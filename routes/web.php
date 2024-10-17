<?php

use Illuminate\Support\Facades\Route;

Route::view('/', 'homepage');
Route::view('/components', 'components');

require __DIR__.'/auth.php';