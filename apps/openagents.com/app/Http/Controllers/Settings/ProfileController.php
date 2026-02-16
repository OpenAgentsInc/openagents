<?php

namespace App\Http\Controllers\Settings;

use App\Http\Controllers\Controller;
use App\Http\Requests\Settings\ProfileUpdateRequest;
use App\Models\User;
use App\Services\PostHogService;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Inertia\Inertia;
use Inertia\Response;
use Laravel\WorkOS\Http\Requests\AuthKitAccountDeletionRequest;

class ProfileController extends Controller
{
    /**
     * Show the user's profile settings page.
     */
    public function edit(Request $request): Response
    {
        return Inertia::render('settings/profile', [
            'status' => $request->session()->get('status'),
        ]);
    }

    /**
     * Update the user's profile settings.
     */
    public function update(ProfileUpdateRequest $request, PostHogService $posthog): RedirectResponse
    {
        $request->user()->update(['name' => $request->name]);

        // PostHog: Track profile updated
        $posthog->capture($request->user()->email, 'profile updated', [
            'field_updated' => 'name',
        ]);

        return to_route('profile.edit');
    }

    /**
     * Delete the user's account.
     */
    public function destroy(AuthKitAccountDeletionRequest $request, PostHogService $posthog): RedirectResponse
    {
        $user = $request->user();
        $userEmail = $user->email;

        // PostHog: Track account deleted before deletion
        $posthog->capture($userEmail, 'account deleted');

        return $request->delete(
            using: fn (User $user) => $user->delete()
        );
    }
}
