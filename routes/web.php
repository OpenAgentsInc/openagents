<?php

use App\Http\Controllers\AuthController;
use App\Http\Controllers\BillingController;
use App\Http\Controllers\CampaignController;
use App\Http\Controllers\DashboardController;
use App\Http\Controllers\ExplorerController;
use App\Http\Controllers\LnAddressController;
use App\Http\Controllers\NostrAuthController;
use App\Http\Controllers\SocialAuthController;
use App\Http\Controllers\Webhook\PoolWebhookReceiver;
use App\Livewire\AdminPanel;
use App\Livewire\Agents\Create;
use App\Livewire\Agents\Edit;
use App\Livewire\Agents\Index;
use App\Livewire\Agents\Profile;
use App\Livewire\Blog;
use App\Livewire\Changelog;
use App\Livewire\Chat;
use App\Livewire\Logs;
use App\Livewire\MarkdownPage;
use App\Livewire\MyAgentsScreen;
use App\Livewire\Plugins\PluginCreate;
use App\Livewire\Plugins\PluginList;
use App\Livewire\ProWelcome;
use App\Livewire\Settings;
use App\Livewire\Store;
use App\Livewire\UserProfile;
use App\Livewire\WalletScreen;
use Illuminate\Support\Facades\Route;
use Laravel\Fortify\Http\Controllers\AuthenticatedSessionController;

// REDIRECT DEFAULT AUTH TO HOME
// Define an array of default authentication routes
$authRoutes = [
//    '/login',
    '/register',
    '/password/reset',
    '/password/reset/{token}',
    '/password/confirm',
    '/password/email',
];

// New auth routes
Route::get('/login', [AuthController::class, 'login_page']);

// Experimental Inertia routes
Route::get('/dashboard', [DashboardController::class, 'dashboard']);
Route::get('/plugin-map', [DashboardController::class, 'plugin_map']);
Route::get('/scratchpad', [DashboardController::class, 'scratchpad']);
Route::get('/test', [DashboardController::class, 'test']);
Route::get('/test2', [DashboardController::class, 'test2']);

// Redirect all authentication routes to home
Route::match(['get', 'post'], '/{authRoute}', function () {
    return redirect()->route('home');
})->whereIn('authRoute', $authRoutes);

// HOME
Route::get('/', function () {
    return redirect()->route('chat');
})->name('home');

// CHAT
Route::get('/chat', Chat::class)->name('chat');
Route::get('/chat/{id}', Chat::class)->name('chat.id');

// AGENTS
Route::get('/agents', Index::class)->name('agents');
Route::get('/create', Create::class)->name('agents.create');
Route::get('/agents/{agent}', Profile::class)->name('agents.profile');
Route::get('/agents/{agent}/edit', Edit::class)->name('agents.edit');
Route::get('/my-agents', MyAgentsScreen::class)->name('myagents');

// LNURLP
Route::get('/.well-known/lnurlp/{user}', [LnAddressController::class, 'handleLnurlp']);
Route::get('/lnurlp/callback', [LnAddressController::class, 'handleCallback']);
Route::get('/payin/{id}', [LnAddressController::class, 'showPayinStatus']);

// PROFILES
Route::get('/u/{username}', UserProfile::class)->name('user.profile');

// STORE
Route::get('/store', Store::class)->name('store');

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

// PLUGIN REGISTRY
Route::get('/plugins', PluginList::class)->name('plugins.index');
Route::get('/plugins/edit/{plugin?}', PluginCreate::class)->name('plugins.edit');

// BLOG
Route::get('/blog', Blog::class);
Route::get('/launch', MarkdownPage::class);
Route::get('/goodbye-chatgpt', MarkdownPage::class);
Route::get('/introducing-the-agent-store', MarkdownPage::class);

// LANDERS
Route::get('/campaign/{id}', [CampaignController::class, 'land']);

// WALLET
Route::get('/wallet', WalletScreen::class)->name('wallet');

// PAYMENTS EXPLORER
Route::get('/explorer', [ExplorerController::class, 'index'])->name('explorer');

// MISC
Route::get('/changelog', Changelog::class);
Route::get('/terms', MarkdownPage::class);
Route::get('/privacy', MarkdownPage::class);
Route::get('/docs', function () {
    return redirect('https://docs.openagents.com');
});

// ADMIN
Route::get('/logs', Logs::class)->name('logs');
Route::get('/admin', AdminPanel::class)->name('admin');

// DEPRECATED: Nostr Webhook
Route::post('/webhook/nostr', [PoolWebhookReceiver::class, 'handleEvent']);

// Pool Webhook
Route::post('/webhook/pool', [PoolWebhookReceiver::class, 'handleEvent']);

// Logout via GET not just POST
Route::get('/logout', [AuthenticatedSessionController::class, 'destroy']);

// Catch-all redirect to the homepage
Route::get('/login', function () {
    return redirect('/');
});

Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
