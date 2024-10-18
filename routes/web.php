<?php

use Illuminate\Support\Facades\Route;
use App\Http\Controllers\MessageController;

// Existing routes...

Route::get('/chat/{thread}/stream', [MessageController::class, 'streamResponse'])->name('chat.stream');