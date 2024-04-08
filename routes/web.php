<?php

use App\Http\Controllers\BillingController;
use App\Http\Controllers\SocialAuthController;
use App\Http\Controllers\StaticController;
use App\Livewire\Auth\ChangePassword;
use App\Livewire\Auth\VerifyAccount;
use App\Livewire\Chat;
use Illuminate\Support\Facades\Route;
use Laravel\Fortify\Http\Controllers\AuthenticatedSessionController;

// CHAT
Route::get('/', Chat::class)->name('home');
Route::get('/chat/{id}', Chat::class)->name('chat');

// AUTH
Route::get('/reset/account/{token}', ChangePassword::class)->name('password.reset');
Route::get('/email/verify/{id}/{hash}', VerifyAccount::class)->name('verification.verify');

// AUTH - SOCIAL
Route::get('/login/x', [SocialAuthController::class, 'login_x']);
Route::get('/callback/x', [SocialAuthController::class, 'login_x_callback']);

// BILLING
Route::get('/subscription', [BillingController::class, 'stripe_billing_portal']);
Route::get('/upgrade', [BillingController::class, 'stripe_subscribe']);

// STATIC
Route::get('/blog', [StaticController::class, 'blog']);
Route::get('/launch', [StaticController::class, 'launch']);
Route::get('/goodbye-chatgpt', [StaticController::class, 'goodbye']);
Route::get('/docs', [StaticController::class, 'docs']);

// Add GET logout route
Route::get('/logout', [AuthenticatedSessionController::class, 'destroy']);

// Catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
