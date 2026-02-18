<?php

namespace App\Http\Controllers\Settings;

use App\Http\Controllers\Controller;
use App\Http\Requests\Settings\ProfileUpdateRequest;
use App\Models\Autopilot;
use App\Models\User;
use App\Services\AutopilotService;
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
     * Show the user's autopilot settings page.
     */
    public function editAutopilot(Request $request): Response
    {
        return Inertia::render('settings/autopilot', [
            'status' => $request->session()->get('status'),
            'autopilotSettings' => $this->autopilotSettingsForUser($request->user()),
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
     * Update (or initialize) the user's default Autopilot profile.
     */
    public function updateAutopilot(Request $request, AutopilotService $autopilotService, PostHogService $posthog): RedirectResponse
    {
        $user = $request->user();

        if (! $user) {
            abort(401);
        }

        $validated = $request->validate([
            'displayName' => ['nullable', 'string', 'max:120'],
            'tagline' => ['nullable', 'string', 'max:255'],
            'ownerDisplayName' => ['nullable', 'string', 'max:120'],
            'personaSummary' => ['nullable', 'string'],
            'autopilotVoice' => ['nullable', 'string', 'max:64'],
            'principlesText' => ['nullable', 'string'],
        ]);

        $autopilot = Autopilot::query()
            ->where('owner_user_id', $user->id)
            ->orderByDesc('updated_at')
            ->first();

        if (! $autopilot) {
            $autopilot = $autopilotService->createForUser($user, [
                'displayName' => (string) ($validated['displayName'] ?? ($user->name.' Autopilot')),
                'visibility' => 'private',
                'status' => 'active',
            ]);
        }

        $principles = [];
        $principlesText = trim((string) ($validated['principlesText'] ?? ''));
        if ($principlesText !== '') {
            $principles = array_values(array_filter(
                array_map(static fn (string $line): string => trim($line), preg_split('/\r\n|\r|\n/', $principlesText) ?: []),
                static fn (string $line): bool => $line !== ''
            ));
        }

        $autopilotService->updateOwned($user, $autopilot->id, [
            'displayName' => $validated['displayName'] ?? $autopilot->display_name,
            'tagline' => $validated['tagline'] ?? $autopilot->tagline,
            'profile' => [
                'ownerDisplayName' => $validated['ownerDisplayName'] ?? $user->name,
                'personaSummary' => $validated['personaSummary'] ?? null,
                'autopilotVoice' => $validated['autopilotVoice'] ?? null,
                'principles' => $principles,
            ],
        ]);

        $posthog->capture($user->email, 'autopilot profile updated', [
            'autopilot_id' => (string) $autopilot->id,
            'source' => 'settings_profile',
        ]);

        return to_route('profile.autopilot.edit')->with('status', 'autopilot-updated');
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

    /**
     * Build the autopilot settings payload for the current user.
     *
     * @return array<string, mixed>|null
     */
    private function autopilotSettingsForUser(?User $user): ?array
    {
        if (! $user) {
            return null;
        }

        $autopilot = Autopilot::query()
            ->with(['profile', 'policy'])
            ->where('owner_user_id', $user->id)
            ->orderByDesc('updated_at')
            ->first();

        if (! $autopilot) {
            return null;
        }

        return [
            'id' => (string) $autopilot->id,
            'handle' => (string) $autopilot->handle,
            'displayName' => (string) $autopilot->display_name,
            'tagline' => $autopilot->tagline,
            'configVersion' => (int) $autopilot->config_version,
            'profile' => $autopilot->profile ? [
                'ownerDisplayName' => $autopilot->profile->owner_display_name,
                'personaSummary' => $autopilot->profile->persona_summary,
                'autopilotVoice' => $autopilot->profile->autopilot_voice,
                'principles' => $autopilot->profile->principles ?? [],
            ] : null,
            'policy' => $autopilot->policy ? [
                'l402RequireApproval' => (bool) $autopilot->policy->l402_require_approval,
                'l402AllowedHosts' => $autopilot->policy->l402_allowed_hosts ?? [],
            ] : null,
        ];
    }
}
