<?php

use App\Http\Controllers\Auth\EmailCodeAuthController;
use App\Http\Controllers\Auth\LocalTestLoginController;
use App\Services\PostHogService;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\Route;

Route::middleware('guest')->group(function () {
    Route::get('login', [EmailCodeAuthController::class, 'show'])
        ->name('login');

    Route::post('login/email', [EmailCodeAuthController::class, 'sendCode'])
        ->middleware('throttle:6,1')
        ->name('login.email');

    Route::post('login/verify', [EmailCodeAuthController::class, 'verifyCode'])
        ->middleware('throttle:10,1')
        ->name('login.verify');

    Route::post('api/auth/email', [EmailCodeAuthController::class, 'sendCodeJson'])
        ->middleware('throttle:6,1')
        ->name('api.auth.email');

    Route::post('api/auth/verify', [EmailCodeAuthController::class, 'verifyCodeJson'])
        ->middleware('throttle:10,1')
        ->name('api.auth.verify');

    Route::get('register', fn () => redirect()->route('login'))
        ->name('register');

    Route::get('authenticate', fn () => redirect()->route('login'));

    if (app()->environment(['local', 'testing'])) {
        Route::get('internal/test-login', LocalTestLoginController::class)
            ->middleware(['signed', 'throttle:30,1'])
            ->name('internal.test-login');
    }
});

Route::post('logout', function (Request $request, PostHogService $posthog) {
    $user = $request->user();

    if ($user) {
        $posthog->capture($user->email, 'user logged out');
    }

    Auth::guard('web')->logout();
    $request->session()->invalidate();
    $request->session()->regenerateToken();

    return redirect()->to(url('/'));
})->middleware('auth')->name('logout');
