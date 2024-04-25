<?php

use App\Livewire\Chat;
use App\Livewire\Admin;
use App\Livewire\Store;
use App\Livewire\Settings;
use App\Livewire\PrismDashboard;
use Illuminate\Support\Facades\Route;
use App\Http\Controllers\StaticController;
use App\Http\Controllers\BillingController;
use App\Http\Controllers\NostrAuthController;
use App\Http\Controllers\SocialAuthController;
use App\Livewire\Explorer;
use App\Http\Controllers\Webhook\NostraHandlerController;
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
Route::get('/store', Store::class)->name('store');

// PLUGIN REGISTRY
Route::get('/plugins', [StaticController::class, 'plugins']);

// AGENT
Route::get('/agents', App\Livewire\Agents\Index::class)->name('agents');
Route::get('/agents/create', App\Livewire\Agents\Create::class)->name('agents.create');
Route::get('/agents/{agent}/edit', App\Livewire\Agents\Edit::class)->name('agents.edit');

// PAYMENTS
Route::get('/prism', PrismDashboard::class)->name('prism');
Route::get('/explorer', Explorer::class)->name('explorer');

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

// ADMIN
Route::get('/admin', Admin::class)->name('admin');



// Nostra Webhook
Route::post('/webhook/nostra',[NostraHandlerController::class,'handleEvent']);

// Catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
