<?php

namespace App\AI\Tools;

use App\AI\Tools\Concerns\L402PaywallToolSupport;
use App\Exceptions\L402\ApertureReconcileException;
use App\Models\L402Paywall;
use App\Services\L402\L402PaywallOperatorService;
use Illuminate\Contracts\JsonSchema\JsonSchema;
use Laravel\Ai\Contracts\Tool;
use Laravel\Ai\Tools\Request;

class LightningL402PaywallDeleteTool implements Tool
{
    use L402PaywallToolSupport;

    public function name(): string
    {
        return 'lightning_l402_paywall_delete';
    }

    public function description(): string
    {
        return 'Delete (soft-delete) an existing L402 seller paywall route (authenticated) with deployment references.';
    }

    public function handle(Request $request): string
    {
        $user = $this->resolveAuthenticatedUser();
        if (! $user) {
            return $this->encodePayload([
                'toolName' => $this->name(),
                'status' => 'failed',
                'denyCode' => 'auth_required',
                'message' => 'Authentication is required to manage paywalls.',
            ]);
        }

        $validated = $this->validateDeletePayload($request->all());
        if ($validated['ok'] !== true) {
            return $this->encodePayload([
                'toolName' => $this->name(),
                'status' => 'blocked',
                'denyCode' => 'validation_failed',
                'errors' => $validated['errors'],
            ]);
        }

        $paywallId = (string) $validated['validated']['paywallId'];

        $paywall = L402Paywall::query()->where('id', $paywallId)->first();
        if (! $paywall) {
            return $this->encodePayload([
                'toolName' => $this->name(),
                'status' => 'failed',
                'denyCode' => 'paywall_not_found',
                'paywallId' => $paywallId,
            ]);
        }

        try {
            $result = resolve(L402PaywallOperatorService::class)->delete($user, $paywall);
        } catch (ApertureReconcileException $exception) {
            $context = $exception->context();

            return $this->encodePayload([
                'toolName' => $this->name(),
                'status' => 'failed',
                'denyCode' => 'reconcile_failed',
                'message' => $exception->getMessage(),
                'reverted' => true,
                'paywallId' => $paywallId,
                'operationId' => $this->operationId('l402_paywall_deleted', $context['mutationEventId'] ?? null),
                'mutationEventId' => is_numeric($context['mutationEventId'] ?? null) ? (int) $context['mutationEventId'] : null,
                'deploymentEventId' => is_numeric($context['gatewayEventId'] ?? null) ? (int) $context['gatewayEventId'] : null,
                'context' => $context,
            ]);
        }

        $deleted = $result['paywall'];
        $deployment = is_array($result['deployment'] ?? null) ? $result['deployment'] : [];
        $mutationEventId = $result['mutationEventId'] ?? null;

        return $this->encodePayload([
            'toolName' => $this->name(),
            'status' => 'completed',
            'operationId' => $this->operationId('l402_paywall_deleted', $mutationEventId),
            'mutationEventId' => is_numeric($mutationEventId) ? (int) $mutationEventId : null,
            'deploymentEventId' => is_numeric($deployment['eventId'] ?? null) ? (int) $deployment['eventId'] : null,
            'deploymentStatus' => is_string($deployment['status'] ?? null) ? $deployment['status'] : 'unknown',
            'paywall' => $this->paywallSummary($deleted),
            'deployment' => $deployment,
        ]);
    }

    public function schema(JsonSchema $schema): array
    {
        return [
            'paywallId' => $schema->string()->required()->description('Existing paywall id to delete.'),
        ];
    }
}
