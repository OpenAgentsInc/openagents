<?php

namespace App\AI\Tools;

use App\AI\Tools\Concerns\L402PaywallToolSupport;
use App\Exceptions\L402\ApertureReconcileException;
use App\Services\L402\L402PaywallOperatorService;
use Illuminate\Contracts\JsonSchema\JsonSchema;
use Laravel\Ai\Contracts\Tool;
use Laravel\Ai\Tools\Request;

class LightningL402PaywallCreateTool implements Tool
{
    use L402PaywallToolSupport;

    public function name(): string
    {
        return 'lightning_l402_paywall_create';
    }

    public function description(): string
    {
        return 'Create an L402 seller paywall route (operator-only). Applies strict guardrails and returns deployment references.';
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

        $validated = $this->validateCreatePayload($request->all());
        if ($validated['ok'] !== true) {
            return $this->encodePayload([
                'toolName' => $this->name(),
                'status' => 'blocked',
                'denyCode' => 'validation_failed',
                'errors' => $validated['errors'],
            ]);
        }

        try {
            $result = resolve(L402PaywallOperatorService::class)->create($user, $validated['validated']);
        } catch (ApertureReconcileException $exception) {
            $context = $exception->context();

            return $this->encodePayload([
                'toolName' => $this->name(),
                'status' => 'failed',
                'denyCode' => 'reconcile_failed',
                'message' => $exception->getMessage(),
                'reverted' => true,
                'operationId' => $this->operationId('l402_paywall_created', $context['mutationEventId'] ?? null),
                'mutationEventId' => is_numeric($context['mutationEventId'] ?? null) ? (int) $context['mutationEventId'] : null,
                'deploymentEventId' => is_numeric($context['gatewayEventId'] ?? null) ? (int) $context['gatewayEventId'] : null,
                'context' => $context,
            ]);
        }

        $paywall = $result['paywall'];
        $deployment = is_array($result['deployment'] ?? null) ? $result['deployment'] : [];
        $mutationEventId = $result['mutationEventId'] ?? null;

        return $this->encodePayload([
            'toolName' => $this->name(),
            'status' => 'completed',
            'operationId' => $this->operationId('l402_paywall_created', $mutationEventId),
            'mutationEventId' => is_numeric($mutationEventId) ? (int) $mutationEventId : null,
            'deploymentEventId' => is_numeric($deployment['eventId'] ?? null) ? (int) $deployment['eventId'] : null,
            'deploymentStatus' => is_string($deployment['status'] ?? null) ? $deployment['status'] : 'unknown',
            'paywall' => $this->paywallSummary($paywall),
            'deployment' => $deployment,
        ]);
    }

    public function schema(JsonSchema $schema): array
    {
        return [
            'name' => $schema->string()->required()->description('Human-readable paywall name.'),
            'hostRegexp' => $schema->string()->required()->description('Regex body used to match hostnames (e.g. ^l402\\.openagents\\.com$).'),
            'pathRegexp' => $schema->string()->required()->description('Regex body used to match request paths; must start with ^/ and cannot be catch-all.'),
            'priceMsats' => $schema->integer()->required()->description('Price in millisats (>=1).'),
            'upstream' => $schema->string()->required()->description('HTTPS upstream URL for the protected resource.'),
            'enabled' => $schema->boolean()->description('Whether this paywall is active. Defaults to true.'),
            'metadata' => $schema->object()->description('Optional operator metadata.'),
        ];
    }
}
