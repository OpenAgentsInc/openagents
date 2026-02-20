<?php

namespace App\Support\Khala;

use App\Models\User;
use Illuminate\Support\Carbon;
use RuntimeException;

class KhalaTokenIssuer
{
    /**
     * @param  array<int, string>  $scope
     * @param  array{workspace_id?: string|null, role?: string|null}  $identityContext
     * @return array<string, mixed>
     */
    public function issueForUser(User $user, array $scope = [], array $identityContext = []): array
    {
        $config = (array) config('khala.token', []);

        if (! (bool) ($config['enabled'] ?? false)) {
            throw new RuntimeException('khala token minting is disabled');
        }

        $signingKey = trim((string) ($config['signing_key'] ?? ''));
        if ($signingKey === '') {
            throw new RuntimeException('khala token signing key is not configured');
        }

        $issuer = trim((string) ($config['issuer'] ?? ''));
        $audience = trim((string) ($config['audience'] ?? ''));
        $subjectPrefix = trim((string) ($config['subject_prefix'] ?? 'user'));
        $keyId = trim((string) ($config['key_id'] ?? ''));
        $claimsVersion = trim((string) ($config['claims_version'] ?? 'oa_khala_claims_v1'));
        $ttlSeconds = (int) ($config['ttl_seconds'] ?? 300);
        $minTtlSeconds = (int) ($config['min_ttl_seconds'] ?? 60);
        $maxTtlSeconds = (int) ($config['max_ttl_seconds'] ?? 900);

        if ($issuer === '' || $audience === '') {
            throw new RuntimeException('khala token issuer and audience must be configured');
        }

        if ($claimsVersion === '') {
            throw new RuntimeException('khala token claims_version must be configured');
        }

        if ($minTtlSeconds <= 0 || $maxTtlSeconds <= 0 || $maxTtlSeconds < $minTtlSeconds) {
            throw new RuntimeException('khala token ttl bounds are invalid');
        }

        if ($ttlSeconds < $minTtlSeconds || $ttlSeconds > $maxTtlSeconds) {
            throw new RuntimeException('khala token ttl_seconds is outside configured bounds');
        }

        $now = Carbon::now('UTC');
        $issuedAt = $now->timestamp;
        $expiresAt = $now->copy()->addSeconds($ttlSeconds)->timestamp;
        $subject = $subjectPrefix.':'.$user->getAuthIdentifier();

        $normalizedScope = $this->normalizeScope($scope);

        $claims = [
            'iss' => $issuer,
            'aud' => $audience,
            'sub' => $subject,
            'iat' => $issuedAt,
            'nbf' => $issuedAt,
            'exp' => $expiresAt,
            'jti' => bin2hex(random_bytes(12)),
            'oa_user_id' => (int) $user->getAuthIdentifier(),
            'oa_claims_version' => $claimsVersion,
        ];

        if ($normalizedScope !== []) {
            $claims['scope'] = $normalizedScope;
        }

        $workspaceId = trim((string) ($identityContext['workspace_id'] ?? ''));
        if ($workspaceId !== '') {
            $claims['oa_workspace_id'] = $workspaceId;
        }

        $role = trim((string) ($identityContext['role'] ?? ''));
        if ($role !== '') {
            $claims['oa_role'] = $role;
        }

        $header = [
            'alg' => 'HS256',
            'typ' => 'JWT',
        ];

        if ($keyId !== '') {
            $header['kid'] = $keyId;
        }

        $token = $this->encodeJwt($header, $claims, $signingKey);

        return [
            'token' => $token,
            'token_type' => 'Bearer',
            'expires_in' => $ttlSeconds,
            'issued_at' => Carbon::createFromTimestampUTC($issuedAt)->toIso8601String(),
            'expires_at' => Carbon::createFromTimestampUTC($expiresAt)->toIso8601String(),
            'issuer' => $issuer,
            'audience' => $audience,
            'subject' => $subject,
            'claims_version' => $claimsVersion,
            'scope' => $normalizedScope,
            'workspace_id' => $workspaceId !== '' ? $workspaceId : null,
            'role' => $role !== '' ? $role : null,
        ];
    }

    /**
     * @param  array<string, mixed>  $header
     * @param  array<string, mixed>  $claims
     */
    private function encodeJwt(array $header, array $claims, string $signingKey): string
    {
        $headerSegment = $this->base64UrlEncode((string) json_encode($header, JSON_THROW_ON_ERROR));
        $claimsSegment = $this->base64UrlEncode((string) json_encode($claims, JSON_THROW_ON_ERROR));
        $signingInput = $headerSegment.'.'.$claimsSegment;

        $signature = hash_hmac('sha256', $signingInput, $signingKey, true);
        $signatureSegment = $this->base64UrlEncode($signature);

        return $signingInput.'.'.$signatureSegment;
    }

    private function base64UrlEncode(string $value): string
    {
        return rtrim(strtr(base64_encode($value), '+/', '-_'), '=');
    }

    /**
     * @param  array<int, string>  $scope
     * @return array<int, string>
     */
    private function normalizeScope(array $scope): array
    {
        return array_values(array_unique(array_filter(
            array_map(static fn ($item): string => trim((string) $item), $scope),
            static fn (string $item): bool => $item !== ''
        )));
    }
}
