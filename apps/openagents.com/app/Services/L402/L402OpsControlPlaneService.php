<?php

namespace App\Services\L402;

use Carbon\Carbon;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;
use InvalidArgumentException;

class L402OpsControlPlaneService
{
    /**
     * @var array<string, int>
     */
    private const INVOICE_STATUS_RANK = [
        'open' => 0,
        'canceled' => 1,
        'expired' => 1,
        'settled' => 2,
    ];

    /**
     * @param  list<string>  $statuses
     * @return list<array<string, mixed>>
     */
    public function listPaywallsForCompile(array $statuses = ['active', 'paused']): array
    {
        $statusSet = collect($statuses)
            ->filter(fn ($status) => is_string($status) && trim($status) !== '')
            ->map(fn (string $status) => trim($status))
            ->values()
            ->all();

        $rows = DB::table('l402_paywalls')
            ->orderBy('created_at')
            ->get();

        $paywalls = [];
        foreach ($rows as $row) {
            $mapped = $this->mapPaywallRow($row);
            if ($statusSet !== [] && ! in_array($mapped['status'], $statusSet, true)) {
                continue;
            }
            $paywalls[] = $mapped;
        }

        return $paywalls;
    }

    /**
     * @param  array<string, mixed>  $input
     * @return array<string, mixed>
     */
    public function recordDeploymentIntent(array $input): array
    {
        $now = now();
        $deploymentId = $this->stringOrNull($input['deploymentId'] ?? null) ?? (string) Str::uuid7();

        $existing = DB::table('l402_control_plane_deployments')
            ->where('deployment_id', $deploymentId)
            ->first();

        $payload = [
            'deployment_id' => $deploymentId,
            'paywall_id' => $this->stringOrNull($input['paywallId'] ?? null),
            'owner_id' => $this->stringOrNull($input['ownerId'] ?? null),
            'config_hash' => $this->requiredString($input['configHash'] ?? null, 'configHash'),
            'image_digest' => $this->stringOrNull($input['imageDigest'] ?? null),
            'status' => $this->requiredString($input['status'] ?? null, 'status'),
            'diagnostics' => $this->jsonDbValue($input['diagnostics'] ?? null),
            'metadata' => $this->jsonDbValue($input['metadata'] ?? null),
            'request_id' => $this->stringOrNull($input['requestId'] ?? null),
            'applied_at_ms' => $this->intOrNull($input['appliedAtMs'] ?? null),
            'rolled_back_from' => $this->stringOrNull($input['rolledBackFrom'] ?? null),
            'updated_at' => $now,
        ];

        if ($existing) {
            DB::table('l402_control_plane_deployments')
                ->where('deployment_id', $deploymentId)
                ->update($payload);
        } else {
            DB::table('l402_control_plane_deployments')->insert($payload + [
                'created_at' => $now,
            ]);
        }

        $row = DB::table('l402_control_plane_deployments')
            ->where('deployment_id', $deploymentId)
            ->first();

        if (! $row) {
            throw new InvalidArgumentException('deployment_persist_failed');
        }

        return $this->mapDeploymentRow($row);
    }

    /**
     * @param  array<string, mixed>  $input
     * @return array<string, mixed>
     */
    public function recordGatewayEvent(array $input): array
    {
        $now = now();
        $eventId = 'evt_'.Str::lower(Str::replace('-', '', (string) Str::uuid7()));

        DB::table('l402_control_plane_gateway_events')->insert([
            'event_id' => $eventId,
            'paywall_id' => $this->requiredString($input['paywallId'] ?? null, 'paywallId'),
            'owner_id' => $this->requiredString($input['ownerId'] ?? null, 'ownerId'),
            'event_type' => $this->requiredString($input['eventType'] ?? null, 'eventType'),
            'level' => $this->requiredString($input['level'] ?? null, 'level'),
            'request_id' => $this->stringOrNull($input['requestId'] ?? null),
            'metadata' => $this->jsonDbValue($input['metadata'] ?? null),
            'created_at' => $now,
            'updated_at' => $now,
        ]);

        $row = DB::table('l402_control_plane_gateway_events')
            ->where('event_id', $eventId)
            ->first();

        if (! $row) {
            throw new InvalidArgumentException('gateway_event_persist_failed');
        }

        return $this->mapGatewayEventRow($row);
    }

    /**
     * @param  array<string, mixed>  input
     * @return array{changed: bool, invoice: array<string, mixed>}
     */
    public function recordInvoiceLifecycle(array $input): array
    {
        $invoiceId = $this->requiredString($input['invoiceId'] ?? null, 'invoiceId');
        $status = $this->requiredString($input['status'] ?? null, 'status');

        if (! array_key_exists($status, self::INVOICE_STATUS_RANK)) {
            throw new InvalidArgumentException('invalid_invoice_status');
        }

        $now = now();
        $nowMs = $now->getTimestampMs();

        $existing = DB::table('l402_control_plane_invoices')
            ->where('invoice_id', $invoiceId)
            ->first();

        $requestedSettledAtMs = $this->intOrNull($input['settledAtMs'] ?? null);

        if (! $existing) {
            $settledAtMs = $status === 'settled' ? ($requestedSettledAtMs ?? $nowMs) : null;
            DB::table('l402_control_plane_invoices')->insert([
                'invoice_id' => $invoiceId,
                'paywall_id' => $this->requiredString($input['paywallId'] ?? null, 'paywallId'),
                'owner_id' => $this->requiredString($input['ownerId'] ?? null, 'ownerId'),
                'amount_msats' => $this->requiredInt($input['amountMsats'] ?? null, 'amountMsats'),
                'status' => $status,
                'payment_hash' => $this->stringOrNull($input['paymentHash'] ?? null),
                'payment_request' => $this->stringOrNull($input['paymentRequest'] ?? null),
                'payment_proof_ref' => $this->stringOrNull($input['paymentProofRef'] ?? null),
                'request_id' => $this->stringOrNull($input['requestId'] ?? null),
                'settled_at_ms' => $settledAtMs,
                'created_at' => $now,
                'updated_at' => $now,
            ]);

            $row = DB::table('l402_control_plane_invoices')
                ->where('invoice_id', $invoiceId)
                ->first();
            if (! $row) {
                throw new InvalidArgumentException('invoice_persist_failed');
            }

            return [
                'changed' => true,
                'invoice' => $this->mapInvoiceRow($row),
            ];
        }

        $existingStatus = (string) ($existing->status ?? 'open');
        $nextStatus = $this->chooseInvoiceStatus($existingStatus, $status);
        $nextPaymentHash = $this->stringOrNull($existing->payment_hash) ?? $this->stringOrNull($input['paymentHash'] ?? null);
        $nextPaymentRequest = $this->stringOrNull($existing->payment_request) ?? $this->stringOrNull($input['paymentRequest'] ?? null);
        $nextPaymentProofRef = $this->stringOrNull($existing->payment_proof_ref) ?? $this->stringOrNull($input['paymentProofRef'] ?? null);
        $nextRequestId = $this->stringOrNull($existing->request_id) ?? $this->stringOrNull($input['requestId'] ?? null);
        $existingSettledAtMs = $this->intOrNull($existing->settled_at_ms);
        $nextSettledAtMs = $nextStatus === 'settled'
            ? ($existingSettledAtMs ?? $requestedSettledAtMs ?? $nowMs)
            : $existingSettledAtMs;

        $updatePayload = [
            'paywall_id' => $this->requiredString($input['paywallId'] ?? null, 'paywallId'),
            'owner_id' => $this->requiredString($input['ownerId'] ?? null, 'ownerId'),
            'amount_msats' => $this->requiredInt($input['amountMsats'] ?? null, 'amountMsats'),
            'status' => $nextStatus,
            'payment_hash' => $nextPaymentHash,
            'payment_request' => $nextPaymentRequest,
            'payment_proof_ref' => $nextPaymentProofRef,
            'request_id' => $nextRequestId,
            'settled_at_ms' => $nextSettledAtMs,
            'updated_at' => $now,
        ];

        $changed = $updatePayload['status'] !== $existingStatus
            || (int) $updatePayload['amount_msats'] !== (int) ($existing->amount_msats ?? 0)
            || $updatePayload['payment_hash'] !== $this->stringOrNull($existing->payment_hash)
            || $updatePayload['payment_request'] !== $this->stringOrNull($existing->payment_request)
            || $updatePayload['payment_proof_ref'] !== $this->stringOrNull($existing->payment_proof_ref)
            || $updatePayload['request_id'] !== $this->stringOrNull($existing->request_id)
            || $updatePayload['settled_at_ms'] !== $this->intOrNull($existing->settled_at_ms)
            || $updatePayload['paywall_id'] !== (string) ($existing->paywall_id ?? '')
            || $updatePayload['owner_id'] !== (string) ($existing->owner_id ?? '');

        DB::table('l402_control_plane_invoices')
            ->where('invoice_id', $invoiceId)
            ->update($updatePayload);

        $row = DB::table('l402_control_plane_invoices')
            ->where('invoice_id', $invoiceId)
            ->first();
        if (! $row) {
            throw new InvalidArgumentException('invoice_persist_failed');
        }

        return [
            'changed' => $changed,
            'invoice' => $this->mapInvoiceRow($row),
        ];
    }

    /**
     * @param  array<string, mixed>  input
     * @return array{existed: bool, settlement: array<string, mixed>, invoice?: array<string, mixed>}
     */
    public function recordSettlement(array $input): array
    {
        $settlementId = $this->requiredString($input['settlementId'] ?? null, 'settlementId');

        $existing = DB::table('l402_control_plane_settlements')
            ->where('settlement_id', $settlementId)
            ->first();

        $invoice = null;
        $invoiceId = $this->stringOrNull($input['invoiceId'] ?? null);
        if ($invoiceId !== null) {
            $invoiceResult = $this->recordInvoiceLifecycle([
                'invoiceId' => $invoiceId,
                'paywallId' => $input['paywallId'] ?? null,
                'ownerId' => $input['ownerId'] ?? null,
                'amountMsats' => $input['amountMsats'] ?? null,
                'status' => 'settled',
                'paymentHash' => $input['paymentHash'] ?? null,
                'requestId' => $input['requestId'] ?? null,
            ]);
            $invoice = $invoiceResult['invoice'];
        }

        if ($existing) {
            $result = [
                'existed' => true,
                'settlement' => $this->mapSettlementRow($existing),
            ];
            if ($invoice !== null) {
                $result['invoice'] = $invoice;
            }

            return $result;
        }

        $paymentProofType = $this->requiredString($input['paymentProofType'] ?? null, 'paymentProofType');
        if ($paymentProofType !== 'lightning_preimage') {
            throw new InvalidArgumentException('invalid_payment_proof_type');
        }

        $preimage = strtolower(trim($this->requiredString($input['paymentProofValue'] ?? null, 'paymentProofValue')));
        if (! preg_match('/^[0-9a-f]+$/', $preimage)) {
            throw new InvalidArgumentException('invalid_preimage');
        }

        $now = now();

        DB::table('l402_control_plane_settlements')->insert([
            'settlement_id' => $settlementId,
            'paywall_id' => $this->requiredString($input['paywallId'] ?? null, 'paywallId'),
            'owner_id' => $this->requiredString($input['ownerId'] ?? null, 'ownerId'),
            'invoice_id' => $invoiceId,
            'amount_msats' => $this->requiredInt($input['amountMsats'] ?? null, 'amountMsats'),
            'payment_proof_ref' => sprintf('lightning_preimage:%s', substr($preimage, 0, 24)),
            'request_id' => $this->stringOrNull($input['requestId'] ?? null),
            'metadata' => $this->jsonDbValue($input['metadata'] ?? null),
            'created_at' => $now,
            'updated_at' => $now,
        ]);

        $row = DB::table('l402_control_plane_settlements')
            ->where('settlement_id', $settlementId)
            ->first();
        if (! $row) {
            throw new InvalidArgumentException('settlement_persist_failed');
        }

        $result = [
            'existed' => false,
            'settlement' => $this->mapSettlementRow($row),
        ];
        if ($invoice !== null) {
            $result['invoice'] = $invoice;
        }

        return $result;
    }

    /**
     * @return array{
     *   global: array<string, mixed>,
     *   ownerControls: list<array<string, mixed>>,
     *   credentialRoles: list<array<string, mixed>>
     * }
     */
    public function getSecurityState(): array
    {
        $global = DB::table('l402_control_plane_security_global')
            ->where('state_id', 'global')
            ->first();

        $globalMapped = $global
            ? $this->mapGlobalSecurityRow($global)
            : [
                'stateId' => 'global',
                'globalPause' => false,
                'updatedAtMs' => 0,
            ];

        $ownerControls = DB::table('l402_control_plane_owner_controls')
            ->orderBy('owner_id')
            ->get()
            ->map(fn ($row) => $this->mapOwnerControlRow($row))
            ->values()
            ->all();

        $credentialRoles = DB::table('l402_control_plane_credential_roles')
            ->orderBy('role')
            ->get()
            ->map(fn ($row) => $this->mapCredentialRoleRow($row))
            ->values()
            ->all();

        return [
            'global' => $globalMapped,
            'ownerControls' => $ownerControls,
            'credentialRoles' => $credentialRoles,
        ];
    }

    /**
     * @param  array<string, mixed>  input
     * @return array<string, mixed>
     */
    public function setGlobalPause(array $input): array
    {
        $active = (bool) ($input['active'] ?? false);
        $now = now();
        $nowMs = $now->getTimestampMs();

        $payload = [
            'global_pause' => $active,
            'deny_reason_code' => $active ? 'global_pause_active' : null,
            'deny_reason' => $active
                ? ($this->stringOrNull($input['reason'] ?? null) ?? 'Global paywall pause is active')
                : null,
            'updated_by' => $this->stringOrNull($input['updatedBy'] ?? null),
            'updated_at_ms' => $nowMs,
            'updated_at' => $now,
        ];

        $existing = DB::table('l402_control_plane_security_global')
            ->where('state_id', 'global')
            ->exists();

        if ($existing) {
            DB::table('l402_control_plane_security_global')
                ->where('state_id', 'global')
                ->update($payload);
        } else {
            DB::table('l402_control_plane_security_global')->insert($payload + [
                'state_id' => 'global',
                'created_at' => $now,
            ]);
        }

        $row = DB::table('l402_control_plane_security_global')
            ->where('state_id', 'global')
            ->first();

        if (! $row) {
            throw new InvalidArgumentException('global_security_persist_failed');
        }

        return $this->mapGlobalSecurityRow($row);
    }

    /**
     * @param  array<string, mixed>  input
     * @return array<string, mixed>
     */
    public function setOwnerKillSwitch(array $input): array
    {
        $ownerId = $this->requiredString($input['ownerId'] ?? null, 'ownerId');
        $active = (bool) ($input['active'] ?? false);
        $now = now();
        $nowMs = $now->getTimestampMs();

        $payload = [
            'kill_switch' => $active,
            'deny_reason_code' => $active ? 'owner_kill_switch_active' : null,
            'deny_reason' => $active
                ? ($this->stringOrNull($input['reason'] ?? null) ?? 'Owner kill switch is active')
                : null,
            'updated_by' => $this->stringOrNull($input['updatedBy'] ?? null),
            'updated_at_ms' => $nowMs,
            'updated_at' => $now,
        ];

        $existing = DB::table('l402_control_plane_owner_controls')
            ->where('owner_id', $ownerId)
            ->exists();

        if ($existing) {
            DB::table('l402_control_plane_owner_controls')
                ->where('owner_id', $ownerId)
                ->update($payload);
        } else {
            DB::table('l402_control_plane_owner_controls')->insert($payload + [
                'owner_id' => $ownerId,
                'created_at' => $now,
            ]);
        }

        $row = DB::table('l402_control_plane_owner_controls')
            ->where('owner_id', $ownerId)
            ->first();
        if (! $row) {
            throw new InvalidArgumentException('owner_control_persist_failed');
        }

        return $this->mapOwnerControlRow($row);
    }

    /**
     * @param  array<string, mixed>  input
     * @return array<string, mixed>
     */
    public function rotateCredentialRole(array $input): array
    {
        $role = $this->requiredString($input['role'] ?? null, 'role');
        $existing = $this->findCredentialRole($role);
        $version = max(1, ($existing['version'] ?? 0) + 1);
        $now = now();
        $nowMs = $now->getTimestampMs();

        $this->upsertCredentialRole($role, [
            'status' => 'rotating',
            'version' => $version,
            'fingerprint' => $this->stringOrNull($input['fingerprint'] ?? null),
            'note' => $this->stringOrNull($input['note'] ?? null),
            'updated_at_ms' => $nowMs,
            'last_rotated_at_ms' => $nowMs,
            'revoked_at_ms' => null,
            'updated_at' => $now,
        ], $now);

        return $this->mapCredentialRoleRowByRole($role);
    }

    /**
     * @param  array<string, mixed>  input
     * @return array<string, mixed>
     */
    public function activateCredentialRole(array $input): array
    {
        $role = $this->requiredString($input['role'] ?? null, 'role');
        $existing = $this->findCredentialRole($role);
        $existingVersion = $existing['version'] ?? 0;
        $existingStatus = $existing['status'] ?? null;
        $version = $existing === null
            ? 1
            : ($existingStatus === 'rotating' ? $existingVersion : max(1, $existingVersion + 1));

        $now = now();
        $nowMs = $now->getTimestampMs();

        $this->upsertCredentialRole($role, [
            'status' => 'active',
            'version' => $version,
            'fingerprint' => $this->stringOrNull($input['fingerprint'] ?? null),
            'note' => $this->stringOrNull($input['note'] ?? null),
            'updated_at_ms' => $nowMs,
            'last_rotated_at_ms' => $nowMs,
            'revoked_at_ms' => null,
            'updated_at' => $now,
        ], $now);

        return $this->mapCredentialRoleRowByRole($role);
    }

    /**
     * @param  array<string, mixed>  input
     * @return array<string, mixed>
     */
    public function revokeCredentialRole(array $input): array
    {
        $role = $this->requiredString($input['role'] ?? null, 'role');
        $existing = $this->findCredentialRole($role);
        $version = max(1, $existing['version'] ?? 1);
        $now = now();
        $nowMs = $now->getTimestampMs();

        $this->upsertCredentialRole($role, [
            'status' => 'revoked',
            'version' => $version,
            'fingerprint' => null,
            'note' => $this->stringOrNull($input['note'] ?? null),
            'updated_at_ms' => $nowMs,
            'last_rotated_at_ms' => null,
            'revoked_at_ms' => $nowMs,
            'updated_at' => $now,
        ], $now);

        return $this->mapCredentialRoleRowByRole($role);
    }

    private function mapPaywallRow(object $row): array
    {
        $status = $this->paywallStatus($row);
        $paywallId = (string) ($row->id ?? '');
        $ownerId = sprintf('owner_%s', (string) ($row->owner_user_id ?? 'unknown'));
        $meta = $this->decodeJson($row->meta ?? null);
        $createdAtMs = $this->timestampMs($row->created_at ?? null);
        $updatedAtMs = $this->timestampMs($row->updated_at ?? null);

        $protocol = parse_url((string) ($row->upstream ?? ''), PHP_URL_SCHEME);
        $routeProtocol = $protocol === 'http' ? 'http' : 'https';
        $defaultTimeoutMs = max(1, (int) config('lightning.operator.control_plane_default_timeout_ms', 6000));
        $defaultPriority = max(0, (int) config('lightning.operator.control_plane_default_priority', 10));

        return [
            'paywallId' => $paywallId,
            'ownerId' => $ownerId,
            'name' => (string) ($row->name ?? ''),
            'status' => $status,
            'createdAtMs' => $createdAtMs,
            'updatedAtMs' => $updatedAtMs,
            'policy' => [
                'paywallId' => $paywallId,
                'ownerId' => $ownerId,
                'pricingMode' => 'fixed',
                'fixedAmountMsats' => (int) ($row->price_msats ?? 0),
                ...($this->intFromArray($meta, 'maxPerRequestMsats') !== null
                    ? ['maxPerRequestMsats' => $this->intFromArray($meta, 'maxPerRequestMsats')]
                    : []),
                ...($this->stringListFromArray($meta, 'allowedHosts') !== null
                    ? ['allowedHosts' => $this->stringListFromArray($meta, 'allowedHosts')]
                    : []),
                ...($this->stringListFromArray($meta, 'blockedHosts') !== null
                    ? ['blockedHosts' => $this->stringListFromArray($meta, 'blockedHosts')]
                    : []),
                ...($this->intFromArray($meta, 'quotaPerMinute') !== null
                    ? ['quotaPerMinute' => $this->intFromArray($meta, 'quotaPerMinute')]
                    : []),
                ...($this->intFromArray($meta, 'quotaPerDay') !== null
                    ? ['quotaPerDay' => $this->intFromArray($meta, 'quotaPerDay')]
                    : []),
                'killSwitch' => false,
                'createdAtMs' => $createdAtMs,
                'updatedAtMs' => $updatedAtMs,
            ],
            'routes' => [[
                'routeId' => sprintf('route_%s', $paywallId),
                'paywallId' => $paywallId,
                'ownerId' => $ownerId,
                'hostPattern' => (string) ($row->host_regexp ?? ''),
                'pathPattern' => (string) ($row->path_regexp ?? ''),
                'upstreamUrl' => (string) ($row->upstream ?? ''),
                'protocol' => $routeProtocol,
                'timeoutMs' => $this->intFromArray($meta, 'timeoutMs') ?? $defaultTimeoutMs,
                'priority' => $this->intFromArray($meta, 'priority') ?? $defaultPriority,
                'createdAtMs' => $createdAtMs,
                'updatedAtMs' => $updatedAtMs,
            ]],
        ];
    }

    private function mapDeploymentRow(object $row): array
    {
        return [
            'deploymentId' => (string) ($row->deployment_id ?? ''),
            ...($this->stringOrNull($row->paywall_id ?? null) !== null ? ['paywallId' => (string) $row->paywall_id] : []),
            ...($this->stringOrNull($row->owner_id ?? null) !== null ? ['ownerId' => (string) $row->owner_id] : []),
            'configHash' => (string) ($row->config_hash ?? ''),
            ...($this->stringOrNull($row->image_digest ?? null) !== null ? ['imageDigest' => (string) $row->image_digest] : []),
            'status' => (string) ($row->status ?? ''),
            ...($this->decodeJson($row->diagnostics ?? null) !== null ? ['diagnostics' => $this->decodeJson($row->diagnostics ?? null)] : []),
            ...($this->intOrNull($row->applied_at_ms ?? null) !== null ? ['appliedAtMs' => $this->intOrNull($row->applied_at_ms ?? null)] : []),
            ...($this->stringOrNull($row->rolled_back_from ?? null) !== null ? ['rolledBackFrom' => (string) $row->rolled_back_from] : []),
            'createdAtMs' => $this->timestampMs($row->created_at ?? null),
            'updatedAtMs' => $this->timestampMs($row->updated_at ?? null),
        ];
    }

    private function mapGatewayEventRow(object $row): array
    {
        return [
            'eventId' => (string) ($row->event_id ?? ''),
            'paywallId' => (string) ($row->paywall_id ?? ''),
            'ownerId' => (string) ($row->owner_id ?? ''),
            'eventType' => (string) ($row->event_type ?? ''),
            'level' => (string) ($row->level ?? ''),
            ...($this->stringOrNull($row->request_id ?? null) !== null ? ['requestId' => (string) $row->request_id] : []),
            ...($this->decodeJson($row->metadata ?? null) !== null ? ['metadata' => $this->decodeJson($row->metadata ?? null)] : []),
            'createdAtMs' => $this->timestampMs($row->created_at ?? null),
        ];
    }

    private function mapInvoiceRow(object $row): array
    {
        return [
            'invoiceId' => (string) ($row->invoice_id ?? ''),
            'paywallId' => (string) ($row->paywall_id ?? ''),
            'ownerId' => (string) ($row->owner_id ?? ''),
            'amountMsats' => (int) ($row->amount_msats ?? 0),
            'status' => (string) ($row->status ?? ''),
            ...($this->stringOrNull($row->payment_hash ?? null) !== null ? ['paymentHash' => (string) $row->payment_hash] : []),
            ...($this->stringOrNull($row->payment_request ?? null) !== null ? ['paymentRequest' => (string) $row->payment_request] : []),
            ...($this->stringOrNull($row->payment_proof_ref ?? null) !== null ? ['paymentProofRef' => (string) $row->payment_proof_ref] : []),
            ...($this->stringOrNull($row->request_id ?? null) !== null ? ['requestId' => (string) $row->request_id] : []),
            'createdAtMs' => $this->timestampMs($row->created_at ?? null),
            'updatedAtMs' => $this->timestampMs($row->updated_at ?? null),
            ...($this->intOrNull($row->settled_at_ms ?? null) !== null ? ['settledAtMs' => $this->intOrNull($row->settled_at_ms ?? null)] : []),
        ];
    }

    private function mapSettlementRow(object $row): array
    {
        return [
            'settlementId' => (string) ($row->settlement_id ?? ''),
            'paywallId' => (string) ($row->paywall_id ?? ''),
            'ownerId' => (string) ($row->owner_id ?? ''),
            ...($this->stringOrNull($row->invoice_id ?? null) !== null ? ['invoiceId' => (string) $row->invoice_id] : []),
            'amountMsats' => (int) ($row->amount_msats ?? 0),
            'paymentProofRef' => (string) ($row->payment_proof_ref ?? ''),
            ...($this->stringOrNull($row->request_id ?? null) !== null ? ['requestId' => (string) $row->request_id] : []),
            ...($this->decodeJson($row->metadata ?? null) !== null ? ['metadata' => $this->decodeJson($row->metadata ?? null)] : []),
            'createdAtMs' => $this->timestampMs($row->created_at ?? null),
        ];
    }

    private function mapGlobalSecurityRow(object $row): array
    {
        return [
            'stateId' => (string) ($row->state_id ?? 'global'),
            'globalPause' => (bool) ($row->global_pause ?? false),
            ...($this->stringOrNull($row->deny_reason_code ?? null) !== null ? ['denyReasonCode' => (string) $row->deny_reason_code] : []),
            ...($this->stringOrNull($row->deny_reason ?? null) !== null ? ['denyReason' => (string) $row->deny_reason] : []),
            ...($this->stringOrNull($row->updated_by ?? null) !== null ? ['updatedBy' => (string) $row->updated_by] : []),
            'updatedAtMs' => $this->intOrNull($row->updated_at_ms ?? null) ?? $this->timestampMs($row->updated_at ?? null),
        ];
    }

    private function mapOwnerControlRow(object $row): array
    {
        return [
            'ownerId' => (string) ($row->owner_id ?? ''),
            'killSwitch' => (bool) ($row->kill_switch ?? false),
            ...($this->stringOrNull($row->deny_reason_code ?? null) !== null ? ['denyReasonCode' => (string) $row->deny_reason_code] : []),
            ...($this->stringOrNull($row->deny_reason ?? null) !== null ? ['denyReason' => (string) $row->deny_reason] : []),
            ...($this->stringOrNull($row->updated_by ?? null) !== null ? ['updatedBy' => (string) $row->updated_by] : []),
            'updatedAtMs' => $this->intOrNull($row->updated_at_ms ?? null) ?? $this->timestampMs($row->updated_at ?? null),
        ];
    }

    private function mapCredentialRoleRow(object $row): array
    {
        return [
            'role' => (string) ($row->role ?? ''),
            'status' => (string) ($row->status ?? ''),
            'version' => (int) ($row->version ?? 1),
            ...($this->stringOrNull($row->fingerprint ?? null) !== null ? ['fingerprint' => (string) $row->fingerprint] : []),
            ...($this->stringOrNull($row->note ?? null) !== null ? ['note' => (string) $row->note] : []),
            'updatedAtMs' => $this->intOrNull($row->updated_at_ms ?? null) ?? $this->timestampMs($row->updated_at ?? null),
            ...($this->intOrNull($row->last_rotated_at_ms ?? null) !== null
                ? ['lastRotatedAtMs' => $this->intOrNull($row->last_rotated_at_ms ?? null)]
                : []),
            ...($this->intOrNull($row->revoked_at_ms ?? null) !== null
                ? ['revokedAtMs' => $this->intOrNull($row->revoked_at_ms ?? null)]
                : []),
        ];
    }

    /**
     * @return array{status: string, version: int}|null
     */
    private function findCredentialRole(string $role): ?array
    {
        $row = DB::table('l402_control_plane_credential_roles')
            ->where('role', $role)
            ->first();

        if (! $row) {
            return null;
        }

        return [
            'status' => (string) ($row->status ?? 'active'),
            'version' => (int) ($row->version ?? 1),
        ];
    }

    /**
     * @param  array<string, mixed>  payload
     */
    private function upsertCredentialRole(string $role, array $payload, \DateTimeInterface $now): void
    {
        $exists = DB::table('l402_control_plane_credential_roles')
            ->where('role', $role)
            ->exists();

        if ($exists) {
            DB::table('l402_control_plane_credential_roles')
                ->where('role', $role)
                ->update($payload);
        } else {
            DB::table('l402_control_plane_credential_roles')->insert($payload + [
                'role' => $role,
                'created_at' => $now,
            ]);
        }
    }

    /**
     * @return array<string, mixed>
     */
    private function mapCredentialRoleRowByRole(string $role): array
    {
        $row = DB::table('l402_control_plane_credential_roles')
            ->where('role', $role)
            ->first();

        if (! $row) {
            throw new InvalidArgumentException('credential_role_persist_failed');
        }

        return $this->mapCredentialRoleRow($row);
    }

    private function paywallStatus(object $row): string
    {
        if ($row->deleted_at !== null) {
            return 'archived';
        }

        return (bool) ($row->enabled ?? false) ? 'active' : 'paused';
    }

    private function chooseInvoiceStatus(string $current, string $incoming): string
    {
        $currentRank = self::INVOICE_STATUS_RANK[$current] ?? 0;
        $incomingRank = self::INVOICE_STATUS_RANK[$incoming] ?? 0;

        return $incomingRank > $currentRank ? $incoming : $current;
    }

    private function requiredString(mixed $value, string $field): string
    {
        $normalized = $this->stringOrNull($value);
        if ($normalized === null) {
            throw new InvalidArgumentException(sprintf('missing_%s', $field));
        }

        return $normalized;
    }

    private function stringOrNull(mixed $value): ?string
    {
        if (! is_string($value)) {
            return null;
        }

        $trimmed = trim($value);

        return $trimmed === '' ? null : $trimmed;
    }

    private function requiredInt(mixed $value, string $field): int
    {
        $normalized = $this->intOrNull($value);
        if ($normalized === null) {
            throw new InvalidArgumentException(sprintf('missing_%s', $field));
        }

        return $normalized;
    }

    private function intOrNull(mixed $value): ?int
    {
        if ($value === null) {
            return null;
        }

        if (is_int($value)) {
            return $value;
        }

        if (is_numeric($value)) {
            $int = (int) $value;
            return (string) $int === (string) $value || (string) (float) $value === (string) $value ? $int : null;
        }

        return null;
    }

    /**
     * @return array<string, mixed>|list<mixed>|null
     */
    private function decodeJson(mixed $value): array|null
    {
        if ($value === null) {
            return null;
        }

        if (is_array($value)) {
            return $value;
        }

        if (is_object($value)) {
            return (array) $value;
        }

        if (is_string($value) && trim($value) !== '') {
            $decoded = json_decode($value, true);
            if (json_last_error() === JSON_ERROR_NONE && (is_array($decoded) || is_object($decoded))) {
                return (array) $decoded;
            }
        }

        return null;
    }

    /**
     * @return array<string, mixed>|list<mixed>|null
     */
    private function jsonOrNull(mixed $value): array|null
    {
        if ($value === null) {
            return null;
        }

        if (is_array($value)) {
            return $value;
        }

        if (is_object($value)) {
            return (array) $value;
        }

        return null;
    }

    private function jsonDbValue(mixed $value): ?string
    {
        $normalized = $this->jsonOrNull($value);
        if ($normalized === null) {
            return null;
        }

        $encoded = json_encode($normalized, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);

        return $encoded === false ? null : $encoded;
    }

    private function timestampMs(mixed $value): int
    {
        if ($value === null) {
            return 0;
        }

        if ($value instanceof \DateTimeInterface) {
            return (int) Carbon::instance(\DateTimeImmutable::createFromInterface($value))->getTimestampMs();
        }

        if (is_string($value) && trim($value) !== '') {
            try {
                return (int) Carbon::parse($value)->getTimestampMs();
            } catch (\Throwable) {
                return 0;
            }
        }

        if (is_numeric($value)) {
            return (int) $value;
        }

        return 0;
    }

    private function intFromArray(?array $payload, string $key): ?int
    {
        if ($payload === null || ! array_key_exists($key, $payload)) {
            return null;
        }

        return $this->intOrNull($payload[$key]);
    }

    /**
     * @return list<string>|null
     */
    private function stringListFromArray(?array $payload, string $key): ?array
    {
        if ($payload === null || ! array_key_exists($key, $payload) || ! is_array($payload[$key])) {
            return null;
        }

        $values = collect($payload[$key])
            ->filter(fn ($value) => is_string($value) && trim($value) !== '')
            ->map(fn (string $value) => trim($value))
            ->values()
            ->all();

        return $values;
    }
}
