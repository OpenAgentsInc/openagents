<?php

namespace App\Http\Controllers\Settings;

use App\Http\Controllers\Controller;
use App\Models\UserIntegration;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Validation\ValidationException;
use Inertia\Inertia;
use Inertia\Response;

class IntegrationController extends Controller
{
    public function edit(Request $request): Response
    {
        $user = $request->user();

        if (! $user) {
            abort(401);
        }

        $resend = UserIntegration::query()
            ->where('user_id', $user->id)
            ->where('provider', 'resend')
            ->first();

        return Inertia::render('settings/integrations', [
            'status' => $request->session()->get('status'),
            'integrations' => [
                'resend' => $this->serializeIntegration($resend),
            ],
        ]);
    }

    public function upsertResend(Request $request): RedirectResponse
    {
        $user = $request->user();

        if (! $user) {
            abort(401);
        }

        $validated = $request->validate([
            'resend_api_key' => ['required', 'string', 'min:8', 'max:4096'],
            'sender_email' => ['nullable', 'email', 'max:255'],
            'sender_name' => ['nullable', 'string', 'max:255'],
        ]);

        $apiKey = (string) $validated['resend_api_key'];

        $integration = UserIntegration::query()->firstOrNew([
            'user_id' => $user->id,
            'provider' => 'resend',
        ]);

        $integration->fill([
            'status' => 'active',
            'encrypted_secret' => $apiKey,
            'secret_fingerprint' => hash('sha256', $apiKey),
            'secret_last4' => substr($apiKey, -4),
            'metadata' => [
                'sender_email' => $validated['sender_email'] ?? null,
                'sender_name' => $validated['sender_name'] ?? null,
            ],
            'connected_at' => now(),
            'disconnected_at' => null,
        ]);

        $integration->save();

        return to_route('settings.integrations.edit')->with('status', 'resend-connected');
    }

    public function disconnectResend(Request $request): RedirectResponse
    {
        $user = $request->user();

        if (! $user) {
            abort(401);
        }

        $integration = UserIntegration::query()
            ->where('user_id', $user->id)
            ->where('provider', 'resend')
            ->first();

        if ($integration) {
            $integration->fill([
                'status' => 'inactive',
                'encrypted_secret' => null,
                'secret_fingerprint' => null,
                'secret_last4' => null,
                'disconnected_at' => now(),
            ]);
            $integration->save();
        }

        return to_route('settings.integrations.edit')->with('status', 'resend-disconnected');
    }

    public function testResend(Request $request): RedirectResponse
    {
        $user = $request->user();

        if (! $user) {
            abort(401);
        }

        $integration = UserIntegration::query()
            ->where('user_id', $user->id)
            ->where('provider', 'resend')
            ->first();

        if (! $integration || $integration->status !== 'active' || ! is_string($integration->encrypted_secret) || trim($integration->encrypted_secret) === '') {
            throw ValidationException::withMessages([
                'resend' => 'Connect an active Resend key before running a test.',
            ]);
        }

        return to_route('settings.integrations.edit')->with('status', 'resend-test-queued');
    }

    /**
     * @return array<string, mixed>
     */
    private function serializeIntegration(?UserIntegration $integration): array
    {
        if (! $integration) {
            return [
                'provider' => 'resend',
                'status' => 'inactive',
                'connected' => false,
                'secretLast4' => null,
                'connectedAt' => null,
                'disconnectedAt' => null,
                'metadata' => [],
            ];
        }

        $hasSecret = is_string($integration->encrypted_secret) && trim($integration->encrypted_secret) !== '';
        $connected = $integration->status === 'active' && $hasSecret;

        return [
            'provider' => (string) $integration->provider,
            'status' => (string) $integration->status,
            'connected' => $connected,
            'secretLast4' => $integration->secret_last4,
            'connectedAt' => $integration->connected_at?->toISOString(),
            'disconnectedAt' => $integration->disconnected_at?->toISOString(),
            'metadata' => $integration->metadata ?? [],
        ];
    }
}
