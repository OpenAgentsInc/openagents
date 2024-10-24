<?php

// Component library routes

use Illuminate\Support\Facades\Route;
use Inertia\Inertia;

Route::get('/components', function () {
    return Inertia::render('ComponentLibrary');
});
