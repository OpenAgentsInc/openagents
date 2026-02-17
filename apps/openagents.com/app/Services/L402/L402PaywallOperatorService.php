<?php

namespace App\Services\L402;

use App\Exceptions\L402\ApertureReconcileException;
use App\Models\L402Paywall;
use App\Models\User;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;

class L402PaywallOperatorService
{
    public function __construct(private readonly ApertureReconciler $reconciler) {}

    /**
     * @param  array<string, mixed>  $attributes
     * @return array{paywall:L402Paywall,deployment:array<string,mixed>,mutationEventId:int}
     */
    public function create(User $actor, array $attributes): array
    {
        $paywall = L402Paywall::query()->create($this->mapCreatePayload($actor, $attributes));

        try {
            $deployment = $this->reconcile($actor, 'create', $paywall->id);

            $paywall->forceFill([
                'last_reconcile_status' => 'succeeded',
                'last_reconcile_error' => null,
                'last_reconciled_at' => now(),
            ])->save();

            $mutationEventId = $this->appendEvent($actor->id, 'l402_paywall_created', [
                'paywallId' => (string) $paywall->id,
                'status' => 'succeeded',
                'reverted' => false,
                'deploymentEventId' => $deployment['eventId'] ?? null,
                'payload' => $this->paywallPayload($paywall),
            ]);

            return [
                'paywall' => $paywall->fresh() ?? $paywall,
                'deployment' => $deployment,
                'mutationEventId' => $mutationEventId,
            ];
        } catch (ApertureReconcileException $exception) {
            $paywallId = (string) $paywall->id;
            $paywall->forceDelete();

            $gatewayEventId = $this->appendEvent($actor->id, 'l402_gateway_event', [
                'action' => 'create',
                'paywallId' => $paywallId,
                'status' => 'failed',
                'error' => $exception->getMessage(),
                'context' => $exception->context(),
            ]);

            $mutationEventId = $this->appendEvent($actor->id, 'l402_paywall_created', [
                'paywallId' => $paywallId,
                'status' => 'failed',
                'reverted' => true,
                'gatewayEventId' => $gatewayEventId,
                'error' => $exception->getMessage(),
            ]);

            throw new ApertureReconcileException(
                'Paywall creation failed during reconcile; change reverted.',
                [
                    'action' => 'create',
                    'paywallId' => $paywallId,
                    'reverted' => true,
                    'gatewayEventId' => $gatewayEventId,
                    'mutationEventId' => $mutationEventId,
                    'reconcile' => $exception->context(),
                ],
                $exception,
            );
        }
    }

    /**
     * @param  array<string, mixed>  $attributes
     * @return array{paywall:L402Paywall,deployment:array<string,mixed>,mutationEventId:int}
     */
    public function update(User $actor, L402Paywall $paywall, array $attributes): array
    {
        $original = [
            'name' => $paywall->name,
            'host_regexp' => $paywall->host_regexp,
            'path_regexp' => $paywall->path_regexp,
            'price_msats' => $paywall->price_msats,
            'upstream' => $paywall->upstream,
            'enabled' => $paywall->enabled,
            'meta' => $paywall->meta,
        ];

        $paywall->fill($this->mapUpdatePayload($attributes));
        $paywall->save();

        try {
            $deployment = $this->reconcile($actor, 'update', $paywall->id);

            $paywall->forceFill([
                'last_reconcile_status' => 'succeeded',
                'last_reconcile_error' => null,
                'last_reconciled_at' => now(),
            ])->save();

            $mutationEventId = $this->appendEvent($actor->id, 'l402_paywall_updated', [
                'paywallId' => (string) $paywall->id,
                'status' => 'succeeded',
                'reverted' => false,
                'deploymentEventId' => $deployment['eventId'] ?? null,
                'payload' => $this->paywallPayload($paywall),
            ]);

            return [
                'paywall' => $paywall->fresh() ?? $paywall,
                'deployment' => $deployment,
                'mutationEventId' => $mutationEventId,
            ];
        } catch (ApertureReconcileException $exception) {
            $paywall->forceFill($original + [
                'last_reconcile_status' => 'failed',
                'last_reconcile_error' => $exception->getMessage(),
                'last_reconciled_at' => now(),
            ])->save();

            $gatewayEventId = $this->appendEvent($actor->id, 'l402_gateway_event', [
                'action' => 'update',
                'paywallId' => (string) $paywall->id,
                'status' => 'failed',
                'error' => $exception->getMessage(),
                'context' => $exception->context(),
            ]);

            $mutationEventId = $this->appendEvent($actor->id, 'l402_paywall_updated', [
                'paywallId' => (string) $paywall->id,
                'status' => 'failed',
                'reverted' => true,
                'gatewayEventId' => $gatewayEventId,
                'error' => $exception->getMessage(),
                'restoredPayload' => $this->paywallPayload($paywall),
            ]);

            throw new ApertureReconcileException(
                'Paywall update failed during reconcile; previous state restored.',
                [
                    'action' => 'update',
                    'paywallId' => (string) $paywall->id,
                    'reverted' => true,
                    'gatewayEventId' => $gatewayEventId,
                    'mutationEventId' => $mutationEventId,
                    'reconcile' => $exception->context(),
                ],
                $exception,
            );
        }
    }

    /**
     * @return array{paywall:L402Paywall,deployment:array<string,mixed>,mutationEventId:int}
     */
    public function delete(User $actor, L402Paywall $paywall): array
    {
        $paywallId = (string) $paywall->id;
        $paywall->delete();

        try {
            $deployment = $this->reconcile($actor, 'delete', $paywallId);

            $deleted = L402Paywall::withTrashed()->findOrFail($paywallId);
            $deleted->forceFill([
                'last_reconcile_status' => 'succeeded',
                'last_reconcile_error' => null,
                'last_reconciled_at' => now(),
            ])->save();

            $mutationEventId = $this->appendEvent($actor->id, 'l402_paywall_deleted', [
                'paywallId' => $paywallId,
                'status' => 'succeeded',
                'reverted' => false,
                'deploymentEventId' => $deployment['eventId'] ?? null,
                'payload' => $this->paywallPayload($deleted),
                'deletedAt' => $deleted->deleted_at?->toISOString(),
            ]);

            return [
                'paywall' => $deleted,
                'deployment' => $deployment,
                'mutationEventId' => $mutationEventId,
            ];
        } catch (ApertureReconcileException $exception) {
            $restored = L402Paywall::withTrashed()->find($paywallId);
            if ($restored && $restored->trashed()) {
                $restored->restore();
            }

            if ($restored) {
                $restored->forceFill([
                    'last_reconcile_status' => 'failed',
                    'last_reconcile_error' => $exception->getMessage(),
                    'last_reconciled_at' => now(),
                ])->save();
            }

            $gatewayEventId = $this->appendEvent($actor->id, 'l402_gateway_event', [
                'action' => 'delete',
                'paywallId' => $paywallId,
                'status' => 'failed',
                'error' => $exception->getMessage(),
                'context' => $exception->context(),
            ]);

            $mutationEventId = $this->appendEvent($actor->id, 'l402_paywall_deleted', [
                'paywallId' => $paywallId,
                'status' => 'failed',
                'reverted' => true,
                'gatewayEventId' => $gatewayEventId,
                'error' => $exception->getMessage(),
                'restored' => $restored !== null,
            ]);

            throw new ApertureReconcileException(
                'Paywall deletion failed during reconcile; delete reverted.',
                [
                    'action' => 'delete',
                    'paywallId' => $paywallId,
                    'reverted' => true,
                    'gatewayEventId' => $gatewayEventId,
                    'mutationEventId' => $mutationEventId,
                    'reconcile' => $exception->context(),
                ],
                $exception,
            );
        }
    }

    /**
     * @return array<string, mixed>
     */
    private function reconcile(User $actor, string $action, string $paywallId): array
    {
        $activePaywalls = L402Paywall::query()
            ->whereNull('deleted_at')
            ->where('enabled', true)
            ->orderBy('created_at')
            ->get();

        $deployment = $this->reconciler->reconcile($activePaywalls);

        $eventId = $this->appendEvent($actor->id, 'l402_gateway_deployment', [
            'action' => $action,
            'paywallId' => $paywallId,
            'status' => $deployment['status'] ?? 'unknown',
            'mode' => $deployment['mode'] ?? null,
            'activePaywallCount' => $deployment['activePaywallCount'] ?? $activePaywalls->count(),
            'configPath' => $deployment['configPath'] ?? null,
            'configSha256' => $deployment['configSha256'] ?? null,
            'command' => $deployment['command'] ?? null,
            'stdout' => $deployment['stdout'] ?? null,
            'stderr' => $deployment['stderr'] ?? null,
            'snapshotVersion' => $deployment['snapshotVersion'] ?? null,
        ]);

        return $deployment + [
            'eventId' => $eventId,
        ];
    }

    /**
     * @param  array<string, mixed>  $attributes
     * @return array<string, mixed>
     */
    private function mapCreatePayload(User $actor, array $attributes): array
    {
        return [
            'owner_user_id' => (int) $actor->id,
            'name' => (string) $attributes['name'],
            'host_regexp' => (string) $attributes['hostRegexp'],
            'path_regexp' => (string) $attributes['pathRegexp'],
            'price_msats' => (int) $attributes['priceMsats'],
            'upstream' => (string) $attributes['upstream'],
            'enabled' => array_key_exists('enabled', $attributes) ? (bool) $attributes['enabled'] : true,
            'meta' => isset($attributes['metadata']) && is_array($attributes['metadata']) ? $attributes['metadata'] : null,
        ];
    }

    /**
     * @param  array<string, mixed>  $attributes
     * @return array<string, mixed>
     */
    private function mapUpdatePayload(array $attributes): array
    {
        $payload = [];

        if (array_key_exists('name', $attributes)) {
            $payload['name'] = (string) $attributes['name'];
        }

        if (array_key_exists('hostRegexp', $attributes)) {
            $payload['host_regexp'] = (string) $attributes['hostRegexp'];
        }

        if (array_key_exists('pathRegexp', $attributes)) {
            $payload['path_regexp'] = (string) $attributes['pathRegexp'];
        }

        if (array_key_exists('priceMsats', $attributes)) {
            $payload['price_msats'] = (int) $attributes['priceMsats'];
        }

        if (array_key_exists('upstream', $attributes)) {
            $payload['upstream'] = (string) $attributes['upstream'];
        }

        if (array_key_exists('enabled', $attributes)) {
            $payload['enabled'] = (bool) $attributes['enabled'];
        }

        if (array_key_exists('metadata', $attributes)) {
            $payload['meta'] = is_array($attributes['metadata']) ? $attributes['metadata'] : null;
        }

        return $payload;
    }

    private function appendEvent(int $userId, string $type, array $payload): int
    {
        return DB::table('run_events')->insertGetId([
            'thread_id' => (string) Str::uuid7(),
            'run_id' => (string) Str::uuid7(),
            'user_id' => $userId,
            'autopilot_id' => null,
            'actor_type' => 'system',
            'actor_autopilot_id' => null,
            'type' => $type,
            'payload' => json_encode($payload),
            'created_at' => now(),
        ]);
    }

    /**
     * @return array<string, mixed>
     */
    private function paywallPayload(L402Paywall $paywall): array
    {
        return [
            'id' => (string) $paywall->id,
            'name' => (string) $paywall->name,
            'hostRegexp' => (string) $paywall->host_regexp,
            'pathRegexp' => (string) $paywall->path_regexp,
            'priceMsats' => (int) $paywall->price_msats,
            'upstream' => (string) $paywall->upstream,
            'enabled' => (bool) $paywall->enabled,
            'metadata' => is_array($paywall->meta) ? $paywall->meta : [],
            'deletedAt' => $paywall->deleted_at?->toISOString(),
            'lastReconcileStatus' => $paywall->last_reconcile_status,
            'lastReconcileError' => $paywall->last_reconcile_error,
            'lastReconciledAt' => $paywall->last_reconciled_at?->toISOString(),
            'createdAt' => $paywall->created_at?->toISOString(),
            'updatedAt' => $paywall->updated_at?->toISOString(),
        ];
    }
}
