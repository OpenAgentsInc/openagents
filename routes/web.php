<?php

use App\Http\Controllers\PrismController;
use App\Http\Controllers\StaticController;
use App\Livewire\Auth\ChangePassword;
use App\Livewire\Auth\ForgetPassword;
use App\Livewire\Auth\PasswordResetLink;
use App\Livewire\Chat;
use App\Livewire\Frontpage;
use App\Livewire\PrismDashboard;
use App\Livewire\ReverbDemo;
use Illuminate\Support\Facades\Route;

Route::get('/', Frontpage::class);
Route::get('/chat/{id}', Chat::class);
Route::get('/prism', PrismDashboard::class);
Route::get('/pro', [StaticController::class, 'pro']);
Route::get('/launch', [StaticController::class, 'launch']);
Route::get('/docs', [StaticController::class, 'docs']);
Route::get('/demo/reverb-chat', ReverbDemo::class);
Route::get('/nwc', [PrismController::class, 'nwc']);

Route::get('/billing', function () {
    return request()->user()?->redirectToBillingPortal() ?? redirect('/');
});

// Add auth route here for Livewire views
Route::get('/reset/account', ForgetPassword::class);
Route::get('/reset/account/link', PasswordResetLink::class);
Route::get('/reset/account/change-password', ChangePassword::class);

// Add a catch-all redirect to the homepage
Route::get('/{any}', function () {
    return redirect('/');
})->where('any', '.*');
