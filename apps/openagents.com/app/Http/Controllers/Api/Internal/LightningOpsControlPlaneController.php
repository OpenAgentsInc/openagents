<?php

namespace App\Http\Controllers\Api\Internal;

use App\Http\Controllers\Controller;
use App\Services\L402\L402OpsControlPlaneService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use InvalidArgumentException;

class LightningOpsControlPlaneController extends Controller
{
    public function query(Request $request, L402OpsControlPlaneService $service): JsonResponse
    {
        [$functionName, $args] = $this->validatedPayload($request);
        $args = $this->assertOpsSecret($args);

        try {
            return match ($functionName) {
                'lightning/ops:listPaywallControlPlaneState' => response()->json([
                    'ok' => true,
                    'paywalls' => $service->listPaywallsForCompile($this->statuses($args)),
                ]),
                'lightning/security:getControlPlaneSecurityState' => response()->json([
                    'ok' => true,
                    ...$service->getSecurityState(),
                ]),
                default => $this->unsupportedFunctionResponse($functionName),
            };
        } catch (InvalidArgumentException $exception) {
            return $this->invalidArgsResponse($exception->getMessage());
        }
    }

    public function mutation(Request $request, L402OpsControlPlaneService $service): JsonResponse
    {
        [$functionName, $args] = $this->validatedPayload($request);
        $args = $this->assertOpsSecret($args);

        try {
            return match ($functionName) {
                'lightning/ops:recordGatewayCompileIntent' => response()->json([
                    'ok' => true,
                    'deployment' => $service->recordDeploymentIntent($args),
                ]),
                'lightning/ops:recordGatewayDeploymentEvent' => response()->json([
                    'ok' => true,
                    'event' => $service->recordGatewayEvent($args),
                ]),
                'lightning/settlements:ingestInvoiceLifecycle' => $this->invoiceLifecycleResponse($service, $args),
                'lightning/settlements:ingestSettlement' => $this->settlementResponse($service, $args),
                'lightning/security:setGlobalPause' => response()->json([
                    'ok' => true,
                    'global' => $service->setGlobalPause($args),
                ]),
                'lightning/security:setOwnerKillSwitch' => response()->json([
                    'ok' => true,
                    'ownerControl' => $service->setOwnerKillSwitch($args),
                ]),
                'lightning/security:rotateCredentialRole' => response()->json([
                    'ok' => true,
                    'role' => $service->rotateCredentialRole($args),
                ]),
                'lightning/security:activateCredentialRole' => response()->json([
                    'ok' => true,
                    'role' => $service->activateCredentialRole($args),
                ]),
                'lightning/security:revokeCredentialRole' => response()->json([
                    'ok' => true,
                    'role' => $service->revokeCredentialRole($args),
                ]),
                default => $this->unsupportedFunctionResponse($functionName),
            };
        } catch (InvalidArgumentException $exception) {
            return $this->invalidArgsResponse($exception->getMessage());
        }
    }

    private function invoiceLifecycleResponse(L402OpsControlPlaneService $service, array $args): JsonResponse
    {
        $result = $service->recordInvoiceLifecycle($args);

        return response()->json([
            'ok' => true,
            'changed' => $result['changed'],
            'invoice' => $result['invoice'],
        ]);
    }

    private function settlementResponse(L402OpsControlPlaneService $service, array $args): JsonResponse
    {
        $result = $service->recordSettlement($args);

        return response()->json([
            'ok' => true,
            'existed' => $result['existed'],
            'settlement' => $result['settlement'],
            ...($result['invoice'] ?? null ? ['invoice' => $result['invoice']] : []),
        ]);
    }

    /**
     * @return array{0:string, 1:array<string,mixed>}
     */
    private function validatedPayload(Request $request): array
    {
        $validated = $request->validate([
            'functionName' => ['required', 'string', 'max:180'],
            'args' => ['required', 'array'],
        ]);

        return [
            (string) $validated['functionName'],
            $validated['args'],
        ];
    }

    /**
     * @param  array<string, mixed>  args
     * @return array<string, mixed>
     */
    private function assertOpsSecret(array $args): array
    {
        $expected = trim((string) config('lightning.operator.ops_secret', ''));
        if ($expected === '') {
            abort(response()->json([
                'error' => [
                    'code' => 'ops_secret_unconfigured',
                    'message' => 'lightning ops secret is not configured',
                ],
            ], 500));
        }

        $provided = isset($args['secret']) && is_string($args['secret']) ? trim($args['secret']) : '';
        if ($provided === '' || ! hash_equals($expected, $provided)) {
            abort(response()->json([
                'error' => [
                    'code' => 'invalid_ops_secret',
                    'message' => 'invalid lightning ops secret',
                ],
            ], 401));
        }

        unset($args['secret']);

        return $args;
    }

    /**
     * @param  array<string,mixed>  args
     * @return list<string>
     */
    private function statuses(array $args): array
    {
        if (! isset($args['statuses']) || ! is_array($args['statuses'])) {
            return ['active', 'paused'];
        }

        return collect($args['statuses'])
            ->filter(fn ($status) => is_string($status) && trim($status) !== '')
            ->map(fn (string $status) => trim($status))
            ->values()
            ->all();
    }

    private function unsupportedFunctionResponse(string $functionName): JsonResponse
    {
        return response()->json([
            'error' => [
                'code' => 'unsupported_function',
                'message' => sprintf('unsupported function: %s', $functionName),
            ],
        ], 404);
    }

    private function invalidArgsResponse(string $reason): JsonResponse
    {
        return response()->json([
            'error' => [
                'code' => 'invalid_arguments',
                'message' => $reason,
            ],
        ], 422);
    }
}
