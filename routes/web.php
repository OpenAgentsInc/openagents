<?php

use App\Http\Controllers\BillingController;
use App\Http\Controllers\NostrAuthController;
use App\Http\Controllers\SocialAuthController;
use App\Http\Controllers\StaticController;
use App\Livewire\Chat;
use App\Livewire\PrismDashboard;
use App\Livewire\Settings;
use Illuminate\Support\Facades\Route;
use Laravel\Fortify\Http\Controllers\AuthenticatedSessionController;

// CHAT
Route::get('/', Chat::class)->name('home');
Route::get('/chat/{id}', Chat::class)->name('chat');

Route::middleware('guest')->group(function () {
    // AUTH - SOCIAL
    Route::get('/login/x', [SocialAuthController::class, 'login_x']);
    Route::get('/callback/x', [SocialAuthController::class, 'login_x_callback']);
});

// AUTH - NOSTR
Route::get('/login/nostr', [NostrAuthController::class, 'client'])->name('loginnostrclient');
Route::post('/login/nostr', [NostrAuthController::class, 'create'])->name('loginnostr');

// SETTINGS
Route::get('/settings', Settings::class)->name('settings');

// BILLING
Route::get('/subscription', [BillingController::class, 'stripe_billing_portal']);
Route::get('/upgrade', [BillingController::class, 'stripe_subscribe']);
Route::get('/pro', [BillingController::class, 'pro'])->name('pro');

// STORE
Route::get('/store');

// PLUGIN REGISTRY
Route::get('/plugins', [StaticController::class, 'plugins']);

// PAYMENTS
Route::get('/prism', PrismDashboard::class)->name('prism');

// BLOG
Route::get('/blog', [StaticController::class, 'blog']);
Route::get('/launch', [StaticController::class, 'launch']);
Route::get('/goodbye-chatgpt', [StaticController::class, 'goodbye']);

// MISC
Route::get('/changelog', [StaticController::class, 'changelog']);
Route::get('/docs', [StaticController::class, 'docs']);
Route::get('/terms', [StaticController::class, 'terms']);
Route::get('/privacy', [StaticController::class, 'privacy']);

// Add GET logout route
Route::get('/logout', [AuthenticatedSessionController::class, 'destroy']);

Route::get('/phpinfo', function () {
    dd(phpinfo());
});

Route::get('/testing', [NostrAuthController::class, 'testing']);

// Catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
