<?php

use App\Http\Controllers\Settings\IntegrationController;
use App\Http\Controllers\Settings\ProfileController;
use App\Http\Middleware\ValidateWorkOSSession;
use Illuminate\Support\Facades\Route;

Route::middleware([
    'auth',
    ValidateWorkOSSession::class,
])->group(function () {
    Route::redirect('settings', '/settings/profile');

    Route::get('settings/profile', [ProfileController::class, 'edit'])->name('profile.edit');
    Route::get('settings/autopilot', [ProfileController::class, 'editAutopilot'])->name('profile.autopilot.edit');
    Route::get('settings/integrations', [IntegrationController::class, 'edit'])->name('settings.integrations.edit');
    Route::patch('settings/profile', [ProfileController::class, 'update'])->name('profile.update');
    Route::patch('settings/autopilot', [ProfileController::class, 'updateAutopilot'])->name('profile.autopilot.update');
    Route::post('settings/integrations/resend', [IntegrationController::class, 'upsertResend'])->name('settings.integrations.resend.upsert');
    Route::delete('settings/integrations/resend', [IntegrationController::class, 'disconnectResend'])->name('settings.integrations.resend.disconnect');
    Route::post('settings/integrations/resend/test', [IntegrationController::class, 'testResend'])->name('settings.integrations.resend.test');
    Route::delete('settings/profile', [ProfileController::class, 'destroy'])->name('profile.destroy');
});
