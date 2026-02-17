<?php

namespace App\AI\Tools;

use App\AI\Tools\Concerns\L402PaywallToolSupport;
use App\Exceptions\L402\ApertureReconcileException;
use App\Models\L402Paywall;
use App\Services\L402\L402PaywallOperatorService;
use Illuminate\Contracts\JsonSchema\JsonSchema;
use Laravel\Ai\Contracts\Tool;
use Laravel\Ai\Tools\Request;

class LightningL402PaywallUpdateTool implements Tool
{
    use L402PaywallToolSupport;

    public function name(): string
    {
        return 'lightning_l402_paywall_update';
    }

    public function description(): string
    {
        return 'Update an existing L402 seller paywall route (operator-only) with strict guardrails and deployment references.';
    }

    public function handle(Request $request): string
    {
        $user = $this->resolveAdminUser();
        if (! $user) {
            return $this->encodePayload([
                'toolName' => $this->name(),
                'status' => 'failed',
                'denyCode' => 'operator_forbidden',
                'message' => 'Only configured operator/admin users may manage paywalls.',
            ]);
        }

        $validated = $this->validateUpdatePayload($request->all());
        if ($validated['ok'] !== true) {
            return $this->encodePayload([
                'toolName' => $this->name(),
                'status' => 'blocked',
                'denyCode' => 'validation_failed',
                'errors' => $validated['errors'],
            ]);
        }

        $payload = $validated['validated'];
        $paywallId = (string) $payload['paywallId'];
        unset($payload['paywallId']);

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
            $result = resolve(L402PaywallOperatorService::class)->update($user, $paywall, $payload);
        } catch (ApertureReconcileException $exception) {
            $context = $exception->context();

            return $this->encodePayload([
                'toolName' => $this->name(),
                'status' => 'failed',
                'denyCode' => 'reconcile_failed',
                'message' => $exception->getMessage(),
                'reverted' => true,
                'paywallId' => $paywallId,
                'operationId' => $this->operationId('l402_paywall_updated', $context['mutationEventId'] ?? null),
                'mutationEventId' => is_numeric($context['mutationEventId'] ?? null) ? (int) $context['mutationEventId'] : null,
                'deploymentEventId' => is_numeric($context['gatewayEventId'] ?? null) ? (int) $context['gatewayEventId'] : null,
                'context' => $context,
            ]);
        }

        $updated = $result['paywall'];
        $deployment = is_array($result['deployment'] ?? null) ? $result['deployment'] : [];
        $mutationEventId = $result['mutationEventId'] ?? null;

        return $this->encodePayload([
            'toolName' => $this->name(),
            'status' => 'completed',
            'operationId' => $this->operationId('l402_paywall_updated', $mutationEventId),
            'mutationEventId' => is_numeric($mutationEventId) ? (int) $mutationEventId : null,
            'deploymentEventId' => is_numeric($deployment['eventId'] ?? null) ? (int) $deployment['eventId'] : null,
            'deploymentStatus' => is_string($deployment['status'] ?? null) ? $deployment['status'] : 'unknown',
            'paywall' => $this->paywallSummary($updated),
            'deployment' => $deployment,
        ]);
    }

    public function schema(JsonSchema $schema): array
    {
        return [
            'paywallId' => $schema->string()->required()->description('Existing paywall id to update.'),
            'name' => $schema->string()->description('Human-readable paywall name.'),
            'hostRegexp' => $schema->string()->description('Regex body used to match hostnames (e.g. ^l402\\.openagents\\.com$).'),
            'pathRegexp' => $schema->string()->description('Regex body used to match request paths; must start with ^/ and cannot be catch-all.'),
            'priceMsats' => $schema->integer()->description('Price in millisats (>=1).'),
            'upstream' => $schema->string()->description('HTTPS upstream URL for the protected resource.'),
            'enabled' => $schema->boolean()->description('Whether this paywall is active.'),
            'metadata' => $schema->object()->description('Optional operator metadata.'),
        ];
    }
}
