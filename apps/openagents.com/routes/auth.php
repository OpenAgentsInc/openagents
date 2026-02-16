<?php

use App\Http\Controllers\Auth\EmailCodeAuthController;
use Illuminate\Support\Facades\Route;
use Laravel\WorkOS\Http\Requests\AuthKitLogoutRequest;

Route::middleware('guest')->group(function () {
    Route::get('login', [EmailCodeAuthController::class, 'show'])
        ->name('login');

    Route::post('login/email', [EmailCodeAuthController::class, 'sendCode'])
        ->middleware('throttle:6,1')
        ->name('login.email');

    Route::post('login/verify', [EmailCodeAuthController::class, 'verifyCode'])
        ->middleware('throttle:10,1')
        ->name('login.verify');

    Route::get('register', fn () => redirect()->route('login'))
        ->name('register');

    Route::get('authenticate', fn () => redirect()->route('login'));
});

Route::post('logout', function (AuthKitLogoutRequest $request) {
    return $request->logout('/');
})->middleware('auth')->name('logout');
