<?php

use App\Http\Controllers\Auth\EmailCodeAuthController;
use App\Http\Controllers\Auth\LocalTestLoginController;
use App\Services\PostHogService;
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

    Route::get('internal/test-login', LocalTestLoginController::class)
        ->middleware(['signed', 'throttle:30,1'])
        ->name('internal.test-login');
});

Route::post('logout', function (AuthKitLogoutRequest $request, PostHogService $posthog) {
    $user = $request->user();

    if ($user) {
        // PostHog: Track logout
        $posthog->capture($user->email, 'user logged out');
    }

    return $request->logout('/');
})->middleware('auth')->name('logout');
