<?php

namespace App\Http\Controllers\Settings;

use App\Http\Controllers\Controller;
use App\Models\CommsDeliveryProjection;
use App\Models\UserIntegration;
use App\Models\UserIntegrationAudit;
use App\Services\IntegrationSecretLifecycleService;
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

        $resendAudit = UserIntegrationAudit::query()
            ->where('user_id', $user->id)
            ->where('provider', 'resend')
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
                'resend' => $this->serializeIntegration($resend),
            ],
            'integrationAudit' => [
                'resend' => $this->serializeAuditLog($resendAudit),
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
