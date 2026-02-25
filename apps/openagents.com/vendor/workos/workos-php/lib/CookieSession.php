<?php

namespace WorkOS;

use WorkOS\Resource\SessionAuthenticationSuccessResponse;
use WorkOS\Resource\SessionAuthenticationFailureResponse;

/**
 * Class CookieSession
 *
 * Handles encrypted session cookies for user authentication and session management.
 * Matches workos-node CookieSession behavior - unsealing and validating sessions.
 */
class CookieSession
{
    /**
     * @var UserManagement
     */
    private $userManagement;

    /**
     * @var string Encrypted session data
     */
    private $sealedSession;

    /**
     * @var string Cookie encryption password
     */
    private $cookiePassword;

    /**
     * Constructor.
     *
     * @param UserManagement $userManagement UserManagement instance
     * @param string $sealedSession Encrypted session cookie data
     * @param string $cookiePassword Password used to decrypt the session
     */
    public function __construct(
        UserManagement $userManagement,
        string $sealedSession,
        string $cookiePassword
    ) {
        $this->userManagement = $userManagement;
        $this->sealedSession = $sealedSession;
        $this->cookiePassword = $cookiePassword;
    }

    /**
     * Authenticates the sealed session and returns user information.
     *
     * @return SessionAuthenticationSuccessResponse|SessionAuthenticationFailureResponse
     * @throws Exception\WorkOSException
     */
    public function authenticate()
    {
        return $this->userManagement->authenticateWithSessionCookie(
            $this->sealedSession,
            $this->cookiePassword
        );
    }

    /**
     * Refreshes an expired session and returns new tokens.
     *
     * Note: This method returns raw tokens. The calling code (e.g., authkit-php)
     * is responsible for sealing the tokens into a new session cookie.
     *
     * @param array $options Options for session refresh
     *   - 'organizationId' (string|null): Organization to scope the session to
     *
     * @return array{SessionAuthenticationSuccessResponse|SessionAuthenticationFailureResponse, array|null}
     *         Returns [response, newTokens] where newTokens contains:
     *         - 'access_token': The new access token
     *         - 'refresh_token': The new refresh token
     *         - 'session_id': The session ID
     *         Returns [failureResponse, null] on error.
     * @throws Exception\WorkOSException
     */
    public function refresh(array $options = [])
    {
        $organizationId = $options['organizationId'] ?? null;

        // First authenticate to get the current session data
        $authResult = $this->authenticate();

        if (!$authResult->authenticated) {
            return [$authResult, null];
        }

        // Tight try/catch for refresh token API call
        try {
            $refreshedAuth = $this->userManagement->authenticateWithRefreshToken(
                WorkOS::getClientId(),
                $authResult->refreshToken,
                null,
                null,
                $organizationId
            );
        } catch (Exception\BaseRequestException $e) {
            $failureResponse = new SessionAuthenticationFailureResponse(
                SessionAuthenticationFailureResponse::REASON_HTTP_ERROR
            );
            return [$failureResponse, null];
        }

        // Build success response
        $successResponse = SessionAuthenticationSuccessResponse::constructFromResponse([
            'authenticated' => true,
            'access_token' => $refreshedAuth->accessToken,
            'refresh_token' => $refreshedAuth->refreshToken,
            'session_id' => $authResult->sessionId,
            'user' => $refreshedAuth->user->raw,
            'organization_id' => $refreshedAuth->organizationId ?? $organizationId,
            'authentication_method' => $authResult->authenticationMethod
        ]);

        // Return raw tokens for the caller to seal
        $newTokens = [
            'access_token' => $refreshedAuth->accessToken,
            'refresh_token' => $refreshedAuth->refreshToken,
            'session_id' => $authResult->sessionId
        ];

        return [$successResponse, $newTokens];
    }

    /**
     * Gets the logout URL for the current session.
     *
     * @param array $options
     *   - 'returnTo' (string|null): URL to redirect to after logout
     *
     * @return string Logout URL
     * @throws Exception\UnexpectedValueException
     */
    public function getLogoutUrl(array $options = [])
    {
        $authResult = $this->authenticate();

        if (!$authResult->authenticated) {
            throw new Exception\UnexpectedValueException(
                "Cannot get logout URL for unauthenticated session"
            );
        }

        $returnTo = $options['returnTo'] ?? null;
        return $this->userManagement->getLogoutUrl($authResult->sessionId, $returnTo);
    }
}
