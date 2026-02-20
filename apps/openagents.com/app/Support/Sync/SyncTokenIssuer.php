<?php

namespace App\Support\Sync;

use App\Models\User;
use Illuminate\Support\Carbon;
use InvalidArgumentException;
use RuntimeException;

class SyncTokenIssuer
{
    /**
     * @param  array<int, string>  $requestedScopes
     * @return array<string, mixed>
     */
    public function issueForUser(User $user, array $requestedScopes = []): array
    {
        $config = (array) config('sync.token', []);

        if (! (bool) ($config['enabled'] ?? false)) {
            throw new RuntimeException('sync token minting is disabled');
        }

        $signingKey = trim((string) ($config['signing_key'] ?? ''));
        if ($signingKey === '') {
            throw new RuntimeException('sync token signing key is not configured');
        }

        $issuer = trim((string) ($config['issuer'] ?? ''));
        $audience = trim((string) ($config['audience'] ?? ''));
        $subjectPrefix = trim((string) ($config['subject_prefix'] ?? 'user'));
        $orgPrefix = trim((string) ($config['org_prefix'] ?? 'user'));
        $keyId = trim((string) ($config['key_id'] ?? ''));
        $claimsVersion = trim((string) ($config['claims_version'] ?? 'oa_sync_claims_v1'));
        $ttlSeconds = (int) ($config['ttl_seconds'] ?? 300);
        $minTtlSeconds = (int) ($config['min_ttl_seconds'] ?? 60);
        $maxTtlSeconds = (int) ($config['max_ttl_seconds'] ?? 900);
        $allowedScopes = $this->normalizeScope((array) ($config['allowed_scopes'] ?? []));
        $defaultScopes = $this->normalizeScope((array) ($config['default_scopes'] ?? []));

        if ($issuer === '' || $audience === '') {
            throw new RuntimeException('sync token issuer and audience must be configured');
        }

        if ($keyId === '') {
            throw new RuntimeException('sync token key_id must be configured');
        }

        if ($claimsVersion === '') {
            throw new RuntimeException('sync token claims_version must be configured');
        }

        if ($minTtlSeconds <= 0 || $maxTtlSeconds <= 0 || $maxTtlSeconds < $minTtlSeconds) {
            throw new RuntimeException('sync token ttl bounds are invalid');
        }

        if ($ttlSeconds < $minTtlSeconds || $ttlSeconds > $maxTtlSeconds) {
            throw new RuntimeException('sync token ttl_seconds is outside configured bounds');
        }

        $normalizedRequestedScopes = $this->normalizeScope($requestedScopes);

        if ($normalizedRequestedScopes !== []) {
            $unknownScopes = array_values(array_diff($normalizedRequestedScopes, $allowedScopes));

            if ($unknownScopes !== []) {
                throw new InvalidArgumentException('requested sync scopes are not allowed');
            }

            $scopes = $normalizedRequestedScopes;
        } else {
            $scopes = $defaultScopes !== [] ? $defaultScopes : $allowedScopes;
        }

        if ($scopes === []) {
            throw new RuntimeException('sync token scopes are not configured');
        }

        $now = Carbon::now('UTC');
        $issuedAt = $now->timestamp;
        $expiresAt = $now->copy()->addSeconds($ttlSeconds)->timestamp;
        $subject = $subjectPrefix.':'.$user->getAuthIdentifier();
        $orgId = $orgPrefix.':'.$user->getAuthIdentifier();

        $claims = [
            'iss' => $issuer,
            'aud' => $audience,
            'sub' => $subject,
            'iat' => $issuedAt,
            'nbf' => $issuedAt,
            'exp' => $expiresAt,
            'jti' => bin2hex(random_bytes(12)),
            'oa_user_id' => (int) $user->getAuthIdentifier(),
            'oa_org_id' => $orgId,
            'oa_sync_scopes' => $scopes,
            'oa_claims_version' => $claimsVersion,
        ];

        $header = [
            'alg' => 'HS256',
            'typ' => 'JWT',
            'kid' => $keyId,
        ];

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
            'org_id' => $orgId,
            'claims_version' => $claimsVersion,
            'scopes' => $scopes,
            'kid' => $keyId,
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
