<?php

use App\Http\Controllers\BillingController;
use App\Http\Controllers\NostrAuthController;
use App\Http\Controllers\NostrGrpcController;
use App\Http\Controllers\SocialAuthController;
use App\Http\Controllers\StaticController;
use App\Http\Controllers\Webhook\NostrHandlerController;
use App\Livewire\Admin;
use App\Livewire\Blog;
use App\Livewire\Changelog;
use App\Livewire\Chat;
use App\Livewire\Explorer;
use App\Livewire\IndexedCodebaseList;
use App\Livewire\MarkdownPage;
use App\Livewire\PrismDashboard;
use App\Livewire\ProWelcome;
use App\Livewire\Settings;
use Illuminate\Support\Facades\Route;
use Laravel\Fortify\Http\Controllers\AuthenticatedSessionController;

//Route::get('/', Store::class)->name('store');

// CHAT
Route::get('/', function () {
    return redirect()->route('chat');
})->name('home');

Route::get('/chat', Chat::class)->name('chat');
Route::get('/chat/{id}', Chat::class)->name('chat.id');

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
Route::get('/pro', ProWelcome::class)->name('pro');

// CODEBASE INDEXES
Route::get('/codebases', IndexedCodebaseList::class);

// PLUGIN REGISTRY
Route::get('/plugins', [StaticController::class, 'plugins']);

// AGENT
Route::get('/agents', App\Livewire\Agents\Index::class)->name('agents');
Route::get('/create', App\Livewire\Agents\Create::class)->name('agents.create');
Route::get('/agents/{agent}/edit', App\Livewire\Agents\Edit::class)->name('agents.edit');

// PAYMENTS
Route::get('/prism', PrismDashboard::class)->name('prism');
Route::get('/explorer', Explorer::class)->name('explorer');

// BLOG
Route::get('/blog', Blog::class);
Route::get('/launch', MarkdownPage::class);
Route::get('/goodbye-chatgpt', MarkdownPage::class);

//GRPC NOSTR
Route::get('/request-job', [NostrGrpcController::class, 'handleJobRequest']);

// MISC
Route::get('/changelog', Changelog::class);
Route::get('/docs', MarkdownPage::class);
Route::get('/terms', MarkdownPage::class);
Route::get('/privacy', MarkdownPage::class);

// Add GET logout route
Route::get('/logout', [AuthenticatedSessionController::class, 'destroy']);

// ADMIN
Route::get('/admin', Admin::class)->name('admin');

// Nostr Webhook
Route::post('/webhook/nostr', [NostrHandlerController::class, 'handleEvent']);

// Catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
