<?php

namespace App\Http\Controllers\Settings;

use App\Http\Controllers\Controller;
use App\Models\CommsDeliveryProjection;
use App\Models\UserIntegration;
use App\Models\UserIntegrationAudit;
use App\Services\IntegrationSecretLifecycleService;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Str;
use Illuminate\Validation\ValidationException;
use Inertia\Inertia;
use Inertia\Response;

class IntegrationController extends Controller
{
    private const GOOGLE_STATE_SESSION_KEY = 'settings.integrations.google_oauth_state';

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

        $google = UserIntegration::query()
            ->where('user_id', $user->id)
            ->where('provider', 'google')
            ->first();

        $resendAudit = UserIntegrationAudit::query()
            ->where('user_id', $user->id)
            ->where('provider', 'resend')
            ->orderByDesc('created_at')
            ->limit(10)
            ->get();

        $googleAudit = UserIntegrationAudit::query()
            ->where('user_id', $user->id)
            ->where('provider', 'google')
            ->orderByDesc('created_at')
            ->limit(10)
            ->get();

        $resendProjection = CommsDeliveryProjection::query()
            ->where('user_id', $user->id)
            ->where('provider', 'resend')
            ->orderByDesc('last_event_at')
            ->orderByDesc('id')
            ->first();

        return Inertia::render('settings/integrations', [
            'status' => $request->session()->get('status'),
            'integrations' => [
                'resend' => $this->serializeIntegration($resend, 'resend'),
                'google' => $this->serializeIntegration($google, 'google'),
            ],
            'integrationAudit' => [
                'resend' => $this->serializeAuditLog($resendAudit),
                'google' => $this->serializeAuditLog($googleAudit),
            ],
            'deliveryProjection' => [
                'resend' => $this->serializeDeliveryProjection($resendProjection),
            ],
        ]);
    }

    public function upsertResend(Request $request, IntegrationSecretLifecycleService $lifecycle): RedirectResponse
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

        $result = $lifecycle->upsertResend($user, $apiKey, [
            'sender_email' => $validated['sender_email'] ?? null,
            'sender_name' => $validated['sender_name'] ?? null,
        ]);

        $status = match ($result['action']) {
            'secret_created' => 'resend-connected',
            'secret_rotated' => 'resend-rotated',
            default => 'resend-updated',
        };

        return to_route('settings.integrations.edit')->with('status', $status);
    }

    public function disconnectResend(Request $request, IntegrationSecretLifecycleService $lifecycle): RedirectResponse
    {
        $user = $request->user();

        if (! $user) {
            abort(401);
        }

        $lifecycle->revokeResend($user);

        return to_route('settings.integrations.edit')->with('status', 'resend-disconnected');
    }

    public function testResend(Request $request, IntegrationSecretLifecycleService $lifecycle): RedirectResponse
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

        $lifecycle->auditTestRequest($user, $integration);

        return to_route('settings.integrations.edit')->with('status', 'resend-test-queued');
    }

    public function redirectGoogle(Request $request): RedirectResponse
    {
        $user = $request->user();

        if (! $user) {
            abort(401);
        }

        $clientId = (string) config('services.google.client_id', '');
        $redirectUri = (string) config('services.google.redirect_uri', '');
        $scopes = trim((string) config('services.google.scopes', ''));

        if ($clientId === '' || $redirectUri === '' || $scopes === '') {
            throw ValidationException::withMessages([
                'google' => 'Google OAuth is not configured on this environment.',
            ]);
        }

        $state = Str::random(64);
        $request->session()->put(self::GOOGLE_STATE_SESSION_KEY, $state);

        $query = http_build_query([
            'client_id' => $clientId,
            'redirect_uri' => $redirectUri,
            'response_type' => 'code',
            'access_type' => 'offline',
            'prompt' => 'consent',
            'scope' => $scopes,
            'state' => $state,
        ]);

        return redirect()->away('https://accounts.google.com/o/oauth2/v2/auth?'.$query);
    }

    public function callbackGoogle(Request $request, IntegrationSecretLifecycleService $lifecycle): RedirectResponse
    {
        $user = $request->user();

        if (! $user) {
            abort(401);
        }

        $oauthError = $request->query('error');
        if (is_string($oauthError) && trim($oauthError) !== '') {
            return to_route('settings.integrations.edit')->withErrors([
                'google' => 'Google authorization failed: '.$oauthError,
            ]);
        }

        $expectedState = (string) $request->session()->pull(self::GOOGLE_STATE_SESSION_KEY, '');
        $incomingState = (string) $request->query('state', '');

        if ($expectedState === '' || $incomingState === '' || ! hash_equals($expectedState, $incomingState)) {
            throw ValidationException::withMessages([
                'google' => 'OAuth state mismatch. Please retry connecting Google.',
            ]);
        }

        $code = (string) $request->query('code', '');
        if ($code === '') {
            throw ValidationException::withMessages([
                'google' => 'Google callback did not include an authorization code.',
            ]);
        }

        $clientId = (string) config('services.google.client_id', '');
        $clientSecret = (string) config('services.google.client_secret', '');
        $redirectUri = (string) config('services.google.redirect_uri', '');

        if ($clientId === '' || $clientSecret === '' || $redirectUri === '') {
            throw ValidationException::withMessages([
                'google' => 'Google OAuth is not configured on this environment.',
            ]);
        }

        $response = Http::asForm()
            ->acceptJson()
            ->timeout(15)
            ->post('https://oauth2.googleapis.com/token', [
                'code' => $code,
                'client_id' => $clientId,
                'client_secret' => $clientSecret,
                'redirect_uri' => $redirectUri,
                'grant_type' => 'authorization_code',
            ]);

        if (! $response->successful()) {
            return to_route('settings.integrations.edit')->withErrors([
                'google' => 'Google token exchange failed. Please reconnect and try again.',
            ]);
        }

        $tokenPayload = $response->json();
        if (! is_array($tokenPayload)) {
            return to_route('settings.integrations.edit')->withErrors([
                'google' => 'Google token exchange returned an invalid payload.',
            ]);
        }

        $result = $lifecycle->upsertGoogle($user, $tokenPayload);

        $status = match ($result['action']) {
            'secret_created' => 'google-connected',
            'secret_rotated' => 'google-rotated',
            default => 'google-updated',
        };

        return to_route('settings.integrations.edit')->with('status', $status);
    }

    public function disconnectGoogle(Request $request, IntegrationSecretLifecycleService $lifecycle): RedirectResponse
    {
        $user = $request->user();

        if (! $user) {
            abort(401);
        }

        $lifecycle->revokeGoogle($user);

        return to_route('settings.integrations.edit')->with('status', 'google-disconnected');
    }

    /**
     * @return array<string, mixed>
     */
    private function serializeIntegration(?UserIntegration $integration, string $provider): array
    {
        if (! $integration) {
            return [
                'provider' => $provider,
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

    /**
     * @return array<int, array<string, mixed>>
     */
    private function serializeAuditLog($audits): array
    {
        return $audits->map(fn (UserIntegrationAudit $audit): array => [
            'action' => (string) $audit->action,
            'createdAt' => $audit->created_at?->toISOString(),
            'metadata' => $audit->metadata ?? [],
        ])->values()->all();
    }

    /**
     * @return array<string, mixed>|null
     */
    private function serializeDeliveryProjection(?CommsDeliveryProjection $projection): ?array
    {
        if (! $projection) {
            return null;
        }

        return [
            'provider' => (string) $projection->provider,
            'integrationId' => (string) $projection->integration_id,
            'lastState' => $projection->last_state,
            'lastEventAt' => $projection->last_event_at?->toISOString(),
            'lastMessageId' => $projection->last_message_id,
            'lastRecipient' => $projection->last_recipient,
            'runtimeEventId' => $projection->runtime_event_id,
            'source' => (string) $projection->source,
        ];
    }
}
