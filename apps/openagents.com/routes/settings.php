<?php

use App\Http\Controllers\Settings\ProfileController;
use App\Http\Middleware\ValidateWorkOSSession;
use Illuminate\Support\Facades\Route;

Route::middleware([
    'auth',
    ValidateWorkOSSession::class,
])->group(function () {
    Route::redirect('settings', '/settings/profile');

    Route::get('settings/profile', [ProfileController::class, 'edit'])->name('profile.edit');
    Route::patch('settings/profile', [ProfileController::class, 'update'])->name('profile.update');
    Route::delete('settings/profile', [ProfileController::class, 'destroy'])->name('profile.destroy');
});
