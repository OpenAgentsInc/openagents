<?php

namespace WorkOS;

/**
 * Class UserManagement
 */
class UserManagement
{
    public const DEFAULT_PAGE_SIZE = 10;
    public const DEFAULT_TOKEN_EXPIRATION = 1440;

    public const AUTHORIZATION_PROVIDER_AUTHKIT = "authkit";
    public const AUTHORIZATION_PROVIDER_APPLE_OAUTH = "AppleOAuth";
    public const AUTHORIZATION_PROVIDER_GITHUB_OAUTH = "GitHubOAuth";
    public const AUTHORIZATION_PROVIDER_GOOGLE_OAUTH = "GoogleOAuth";
    public const AUTHORIZATION_PROVIDER_MICROSOFT_OAUTH = "MicrosoftOAuth";

    /**
     * @var Session\SessionEncryptionInterface|null
     */
    private $sessionEncryptor = null;

    /**
     * @param Session\SessionEncryptionInterface|null $encryptor Optional encryption provider
     */
    public function __construct(?Session\SessionEncryptionInterface $encryptor = null)
    {
        $this->sessionEncryptor = $encryptor;
    }

    /**
     * Set the session encryptor.
     *
     * @param Session\SessionEncryptionInterface $encryptor
     * @return void
     */
    public function setSessionEncryptor(Session\SessionEncryptionInterface $encryptor): void
    {
        $this->sessionEncryptor = $encryptor;
    }

    /**
     * Get the session encryptor, defaulting to Halite.
     *
     * @return Session\SessionEncryptionInterface
     */
    private function getSessionEncryptor(): Session\SessionEncryptionInterface
    {
        if ($this->sessionEncryptor === null) {
            $this->sessionEncryptor = new Session\HaliteSessionEncryption();
        }
        return $this->sessionEncryptor;
    }

    /**
     * Create User.
     *
     * @param string $email The email address of the user.
     * @param string|null $password The password of the user.
     * @param string|null $firstName The first name of the user.
     * @param string|null $lastName The last name of the user.
     * @param boolean|null $emailVerified A boolean declaring if the user's email has been verified.
     * @param string|null $passwordHash The hashed password to set for the user.
     * @param string|null $passwordHashType The algorithm originally used to hash the password. Valid values are `bcrypt`, `ssha`, and `firebase-scrypt`.
     * @param string|null $externalId The user's external ID.
     * @param array<string, string> $metadata The user's metadata.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\User
     */

    public function createUser(
        $email,
        ?string $password = null,
        ?string $firstName = null,
        ?string $lastName = null,
        ?bool $emailVerified = null,
        ?string $passwordHash = null,
        ?string $passwordHashType = null,
        ?string $externalId = null,
        ?array $metadata = null
    ) {
        $path = "user_management/users";
        $params = [
            "email" => $email,
            "password" => $password,
            "first_name" => $firstName,
            "last_name" => $lastName,
            "email_verified" => $emailVerified,
            "password_hash" => $passwordHash,
            "password_hash_type" => $passwordHashType,
            "external_id" => $externalId,
            "metadata" => $metadata
        ];

        $response = Client::request(Client::METHOD_POST, $path, null, $params, true);

        return Resource\User::constructFromResponse($response);
    }

    /**
     * Get a User.
     *
     * @param string $userId user ID
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\User
     */
    public function getUser($userId)
    {
        $path = "user_management/users/{$userId}";

        $response = Client::request(Client::METHOD_GET, $path, null, null, true);

        return Resource\User::constructFromResponse($response);
    }

    /**
     * Get a User by external ID.
     *
     * @param string $externalId The external ID of the user.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\User
     */
    public function getUserByExternalId($externalId)
    {
        $path = "user_management/users/external_id/{$externalId}";

        $response = Client::request(Client::METHOD_GET, $path, null, null, true);

        return Resource\User::constructFromResponse($response);
    }

    /**
     * Update a User
     *
     * @param string $userId The unique ID of the user.
     * @param string|null $firstName The first name of the user.
     * @param string|null $lastName The last name of the user.
     * @param boolean|null $emailVerified A boolean declaring if the user's email has been verified.
     * @param string|null $password The password to set for the user.
     * @param string|null $passwordHash The hashed password to set for the user.
     * @param string|null $passwordHashType The algorithm originally used to hash the password. Valid values are `bcrypt`, `ssha`, and `firebase-scrypt`.
     * @param string|null $externalId The user's external ID.
     * @param array<string, string>|null $metadata The user's metadata.
     * @param string|null $email The email address of the user.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\User
     */
    public function updateUser(
        $userId,
        ?string $firstName = null,
        ?string $lastName = null,
        ?bool $emailVerified = null,
        ?string $password = null,
        ?string $passwordHash = null,
        ?string $passwordHashType = null,
        ?string $externalId = null,
        ?array $metadata = null,
        ?string $email = null
    ) {
        $path = "user_management/users/{$userId}";

        $params = [
            "first_name" => $firstName,
            "last_name" => $lastName,
            "email_verified" => $emailVerified,
            "password" => $password,
            "password_hash" => $passwordHash,
            "password_hash_type" => $passwordHashType,
            "external_id" => $externalId,
            "metadata" => $metadata,
            "email" => $email
        ];

        $response = Client::request(Client::METHOD_PUT, $path, null, $params, true);

        return Resource\User::constructFromResponse($response);
    }

    /**
     * List Users.
     *
     * @param null|string $email
     * @param null|string $organizationId Organization users are a member of
     * @param int $limit Maximum number of records to return
     * @param null|string $before User ID to look before
     * @param null|string $after User ID to look after
     * @param Resource\Order $order The Order in which to paginate records
     *
     * @return array{?string, ?string, Resource\User[]} An array containing the Directory User ID to use as before and after cursor, and an array of User instances
     *
     * @throws Exception\WorkOSException
     */
    public function listUsers(
        ?string $email = null,
        ?string $organizationId = null,
        $limit = self::DEFAULT_PAGE_SIZE,
        ?string $before = null,
        ?string $after = null,
        ?string $order = null
    ) {
        $path = "user_management/users";

        $params = [
            "email" => $email,
            "organization_id" => $organizationId,
            "limit" => $limit,
            "before" => $before,
            "after" => $after,
            "order" => $order
        ];

        $response = Client::request(
            Client::METHOD_GET,
            $path,
            null,
            $params,
            true
        );

        $users = [];
        list($before, $after) = Util\Request::parsePaginationArgs($response);
        foreach ($response["data"] as $responseData) {
            \array_push($users, Resource\User::constructFromResponse($responseData));
        }

        return [$before, $after, $users];
    }

    /**
     * Delete a user.
     *
     * @param string $userId Unique ID of a user
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\Response
     */
    public function deleteUser($userId)
    {
        $path = "user_management/users/{$userId}";

        $response = Client::request(Client::METHOD_DELETE, $path, null, null, true);

        return $response;
    }

    /**
     * Add a User to an Organization.
     *
     * @param string $userId User ID
     * @param string $organizationId Organization ID
     * @param string|null $roleSlug Role Slug
     * @param array|null $roleSlugs Role Slugs
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\OrganizationMembership
     */
    public function createOrganizationMembership($userId, $organizationId, ?string $roleSlug = null, ?array $roleSlugs = null)
    {
        $path = "user_management/organization_memberships";

        $params = [
            "organization_id" => $organizationId,
            "user_id" => $userId
        ];

        if (!is_null($roleSlug)) {
            $params["role_slug"] = $roleSlug;
        }

        if (!is_null($roleSlugs)) {
            $params["role_slugs"] = $roleSlugs;
        }

        $response = Client::request(
            Client::METHOD_POST,
            $path,
            null,
            $params,
            true
        );

        return Resource\OrganizationMembership::constructFromResponse($response);
    }

    /**
     * Get an Organization Membership.
     *
     * @param string $organizationMembershipId Organization Membership ID
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\OrganizationMembership
     */
    public function getOrganizationMembership($organizationMembershipId)
    {
        $path = "user_management/organization_memberships/{$organizationMembershipId}";

        $response = Client::request(
            Client::METHOD_GET,
            $path,
            null,
            null,
            true
        );

        return Resource\OrganizationMembership::constructFromResponse($response);
    }

    /**
     * Remove a user from an organization.
     *
     * @param string $organizationMembershipId Organization Membership ID
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\Response
     */
    public function deleteOrganizationMembership($organizationMembershipId)
    {
        $path = "user_management/organization_memberships/{$organizationMembershipId}";

        $response = Client::request(
            Client::METHOD_DELETE,
            $path,
            null,
            null,
            true
        );

        return $response;
    }

    /**
     * Update a User organization membership.
     *
     * @param string $organizationMembershipId Organization Membership ID
     * @param string|null $role_slug The unique slug of the role to grant to this membership.
     * @param array|null $role_slugs The unique slugs of the roles to grant to this membership.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\OrganizationMembership
     */
    public function updateOrganizationMembership($organizationMembershipId, ?string $roleSlug = null, ?array $roleSlugs = null)
    {
        $path = "user_management/organization_memberships/{$organizationMembershipId}";

        $params = [];

        if (!is_null($roleSlug)) {
            $params["role_slug"] = $roleSlug;
        }

        if (!is_null($roleSlugs)) {
            $params["role_slugs"] = $roleSlugs;
        }

        $response = Client::request(
            Client::METHOD_PUT,
            $path,
            null,
            $params,
            true
        );

        return Resource\OrganizationMembership::constructFromResponse($response);
    }

    /**
     * List organization memberships.
     *
     * @param string|null $userId User ID
     * @param string|null $organizationId Organization ID
     * @param array|null $statuses Organization Membership statuses to filter
     * @param int $limit Maximum number of records to return
     * @param string|null $before Organization Membership ID to look before
     * @param string|null $after Organization Membership ID to look after
     * @param Resource\Order $order The Order in which to paginate records
     *
     * @throws Exception\WorkOSException
     *
     * @return array{?string, ?string, Resource\OrganizationMembership[]} An array containing the Organization Membership ID to use as before and after cursor, and a list of Organization Memberships instances
     */
    public function listOrganizationMemberships(
        ?string $userId = null,
        ?string $organizationId = null,
        ?array $statuses = null,
        $limit = self::DEFAULT_PAGE_SIZE,
        ?string $before = null,
        ?string $after = null,
        ?string $order = null
    ) {
        $path = "user_management/organization_memberships";

        if (isset($statuses)) {
            if (!is_array($statuses)) {
                $msg = "Invalid argument: statuses must be an array or null.";
                throw new Exception\UnexpectedValueException($msg);
            }

            $statuses = join(",", $statuses);
        }

        $params = [
            "organization_id" => $organizationId,
            "user_id" => $userId,
            "statuses" => $statuses,
            "limit" => $limit,
            "before" => $before,
            "after" => $after,
            "order" => $order
        ];

        $response = Client::request(
            Client::METHOD_GET,
            $path,
            null,
            $params,
            true
        );

        $organizationMemberships = [];

        foreach ($response["data"] as $responseData) {
            \array_push($organizationMemberships, Resource\OrganizationMembership::constructFromResponse($responseData));
        }

        list($before, $after) = Util\Request::parsePaginationArgs($response);

        return [$before, $after, $organizationMemberships];
    }

    /**
     * Deactivate an Organization Membership.
     *
     * @param string $organizationMembershipId Organization Membership ID
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\OrganizationMembership
     */
    public function deactivateOrganizationMembership($organizationMembershipId)
    {
        $path = "user_management/organization_memberships/{$organizationMembershipId}/deactivate";

        $response = Client::request(
            Client::METHOD_PUT,
            $path,
            null,
            null,
            true
        );

        return Resource\OrganizationMembership::constructFromResponse($response);
    }

    /**
     * Reactivate an Organization Membership.
     *
     * @param string $organizationMembershipId Organization Membership ID
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\OrganizationMembership
     */
    public function reactivateOrganizationMembership($organizationMembershipId)
    {
        $path = "user_management/organization_memberships/{$organizationMembershipId}/reactivate";

        $response = Client::request(
            Client::METHOD_PUT,
            $path,
            null,
            null,
            true
        );

        return Resource\OrganizationMembership::constructFromResponse($response);
    }

    /**
     * Sends an Invitation
     *
     * @param string $email The email address of the invitee
     * @param string|null $organizationId Organization ID
     * @param int|null $expiresInDays expiration delay in days
     * @param string|null $inviterUserId User ID of the inviter
     * @param string|null $roleSlug Slug of the role to assign to the invitee User
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\Invitation
     */
    public function sendInvitation(
        $email,
        ?string $organizationId = null,
        ?int $expiresInDays = null,
        ?string $inviterUserId = null,
        ?string $roleSlug = null
    ) {
        $path = "user_management/invitations";

        $params = [
            "email" => $email,
            "organization_id" => $organizationId,
            "expires_in_days" => $expiresInDays,
            "inviter_user_id" => $inviterUserId,
            "role_slug" => $roleSlug
        ];

        $response = Client::request(
            Client::METHOD_POST,
            $path,
            null,
            $params,
            true
        );

        return Resource\Invitation::constructFromResponse($response);
    }

    /**
     * Get an Invitation
     *
     * @param string $invitationId ID of the Invitation
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\Invitation
     */
    public function getInvitation($invitationId)
    {
        $path = "user_management/invitations/{$invitationId}";

        $response = Client::request(
            Client::METHOD_GET,
            $path,
            null,
            null,
            true
        );

        return Resource\Invitation::constructFromResponse($response);
    }

    /**
     * Find an Invitation by Token
     *
     * @param string $invitationToken The token of the Invitation
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\Invitation
     */
    public function findInvitationByToken($invitationToken)
    {
        $path = "user_management/invitations/by_token/{$invitationToken}";

        $response = Client::request(
            Client::METHOD_GET,
            $path,
            null,
            null,
            true
        );

        return Resource\Invitation::constructFromResponse($response);
    }

    /**
     * List Invitations
     *
     * @param string|null $email Email of the invitee
     * @param string|null $organizationId Organization ID
     * @param int $limit Maximum number of records to return
     * @param string|null $before Organization Membership ID to look before
     * @param string|null $after Organization Membership ID to look after
     * @param Resource\Order $order The Order in which to paginate records
     *
     * @throws Exception\WorkOSException
     *
     * @return array{?string, ?string, Resource\Invitation[]} An array containing the Invitation ID to use as before and after cursor, and a list of Invitations instances
     */
    public function listInvitations(
        ?string $email = null,
        ?string $organizationId = null,
        $limit = self::DEFAULT_PAGE_SIZE,
        ?string $before = null,
        ?string $after = null,
        ?string $order = null
    ) {
        $path = "user_management/invitations";

        $params = [
            "email" => $email,
            "organization_id" => $organizationId,
            "limit" => $limit,
            "before" => $before,
            "after" => $after,
            "order" => $order
        ];

        $response = Client::request(
            Client::METHOD_GET,
            $path,
            null,
            $params,
            true
        );

        $invitations = [];

        foreach ($response["data"] as $responseData) {
            \array_push($invitations, Resource\Invitation::constructFromResponse($responseData));
        }

        list($before, $after) = Util\Request::parsePaginationArgs($response);

        return [$before, $after, $invitations];
    }

    /**
     * Revoke an Invitation
     *
     * @param string $invitationId ID of the Invitation
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\Invitation
     */
    public function revokeInvitation($invitationId)
    {
        $path = "user_management/invitations/{$invitationId}/revoke";

        $response = Client::request(
            Client::METHOD_POST,
            $path,
            null,
            null,
            true
        );

        return Resource\Invitation::constructFromResponse($response);
    }

    /**
     * Resend an Invitation
     *
     * @param string $invitationId ID of the Invitation
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\Invitation
     */
    public function resendInvitation($invitationId)
    {
        $path = "user_management/invitations/{$invitationId}/resend";

        $response = Client::request(
            Client::METHOD_POST,
            $path,
            null,
            null,
            true
        );

        return Resource\Invitation::constructFromResponse($response);
    }

    /**
     * Generates an OAuth 2.0 authorization URL used to initiate the SSO flow with WorkOS.
     *
     * @param string $redirectUri URI to direct the user to upon successful completion of SSO
     * @param null|array $state Associative array containing state that will be returned from WorkOS as a json encoded string
     * @param null|string $provider Service provider that handles the identity of the user
     * @param null|string $connectionId Unique identifier for a WorkOS Connection
     * @param null|string $organizationId Unique identifier for a WorkOS Organization
     * @param null|string $domainHint Domain hint that will be passed as a parameter to the IdP login page
     * @param null|string $loginHint Username/email hint that will be passed as a parameter to the to IdP login page
     * @param null|string $screenHint The page that the user will be redirected to when the provider is authkit
     * @param null|array $providerScopes An array of provider-specific scopes
     *
     * @throws Exception\UnexpectedValueException
     * @throws Exception\ConfigurationException
     *
     * @return string
     */
    public function getAuthorizationUrl(
        $redirectUri,
        $state = null,
        $provider = null,
        ?string $connectionId = null,
        ?string $organizationId = null,
        ?string $domainHint = null,
        ?string $loginHint = null,
        ?string $screenHint = null,
        ?array $providerScopes = null
    ) {
        $path = "user_management/authorize";

        if (!isset($provider) && !isset($connectionId) && !isset($organizationId)) {
            $msg = "Either \$provider, \$connectionId, or \$organizationId is required";
            throw new Exception\UnexpectedValueException($msg);
        }

        $supportedProviders = [
            self::AUTHORIZATION_PROVIDER_AUTHKIT,
            self::AUTHORIZATION_PROVIDER_APPLE_OAUTH,
            self::AUTHORIZATION_PROVIDER_GITHUB_OAUTH,
            self::AUTHORIZATION_PROVIDER_GOOGLE_OAUTH,
            self::AUTHORIZATION_PROVIDER_MICROSOFT_OAUTH
        ];

        if (isset($provider) && !\in_array($provider, $supportedProviders)) {
            $msg = "Only " . implode("','", $supportedProviders) . " providers are supported";
            throw new Exception\UnexpectedValueException($msg);
        }

        $params = [
            "client_id" => WorkOS::getClientId(),
            "response_type" => "code"
        ];

        if ($redirectUri) {
            $params["redirect_uri"] = $redirectUri;
        }

        if (null !== $state && !empty($state)) {
            $params["state"] = \json_encode($state);
        }

        if ($provider) {
            $params["provider"] = $provider;
        }

        if ($connectionId) {
            $params["connection_id"] = $connectionId;
        }

        if ($organizationId) {
            $params["organization_id"] = $organizationId;
        }

        if ($domainHint) {
            $params["domain_hint"] = $domainHint;
        }

        if ($loginHint) {
            $params["login_hint"] = $loginHint;
        }

        if ($screenHint !== null) {
            if ($provider !== self::AUTHORIZATION_PROVIDER_AUTHKIT) {
                throw new Exception\UnexpectedValueException("A 'screenHint' can only be provided when the provider is 'authkit'.");
            }
            $params["screen_hint"] = $screenHint;
        }

        if ($providerScopes && is_array($providerScopes)) {
            $params["provider_scopes"] = implode(",", $providerScopes);
        }

        return Client::generateUrl($path, $params);
    }

    /**
     * Authenticate a User with Password
     *
     * @param string $clientId This value can be obtained from the API Keys page in the WorkOS dashboard.
     * @param string $email The email address of the user.
     * @param string $password The password of the user.
     * @param string|null $ipAddress The IP address of the request from the user who is attempting to authenticate.
     * @param string|null $userAgent The user agent of the request from the user who is attempting to authenticate.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\AuthenticationResponse
     */
    public function authenticateWithPassword($clientId, $email, $password, ?string $ipAddress = null, ?string $userAgent = null)
    {
        $path = "user_management/authenticate";
        $params = [
            "client_id" => $clientId,
            "email" => $email,
            "password" => $password,
            "ip_address" => $ipAddress,
            "user_agent" => $userAgent,
            "grant_type" => "password",
            "client_secret" => WorkOS::getApiKey()
        ];

        $response = Client::request(Client::METHOD_POST, $path, null, $params, true);

        return Resource\AuthenticationResponse::constructFromResponse($response);
    }

    /**
     * Authenticate a User with Selected Organization
     *
     * @param string $clientId This value can be obtained from the API Keys page in the WorkOS dashboard.
     * @param string $pendingAuthenticationToken Token returned from a failed authentication attempt due to organization selection being required.
     * @param string $organizationId The Organization ID the user selected.
     * @param string|null $ipAddress The IP address of the request from the user who is attempting to authenticate.
     * @param string|null $userAgent The user agent of the request from the user who is attempting to authenticate.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\AuthenticationResponse
     */
    public function authenticateWithSelectedOrganization(
        $clientId,
        $pendingAuthenticationToken,
        $organizationId,
        ?string $ipAddress = null,
        ?string $userAgent = null
    ) {
        $path = "user_management/authenticate";
        $params = [
            "client_id" => $clientId,
            "pending_authentication_token" => $pendingAuthenticationToken,
            "organization_id" => $organizationId,
            "ip_address" => $ipAddress,
            "user_agent" => $userAgent,
            "grant_type" => "urn:workos:oauth:grant-type:organization-selection",
            "client_secret" => WorkOS::getApiKey()
        ];

        $response = Client::request(Client::METHOD_POST, $path, null, $params, true);

        return Resource\AuthenticationResponse::constructFromResponse($response);
    }

    /**
     * Authenticate an OAuth or SSO User with a Code
     * This should be used for "Hosted AuthKit" and "OAuth or SSO" UserAuthentications
     *
     * @param string $clientId This value can be obtained from the API Keys page in the WorkOS dashboard.
     * @param string $code The authorization value which was passed back as a query parameter in the callback to the Redirect URI.
     * @param string|null $ipAddress The IP address of the request from the user who is attempting to authenticate.
     * @param string|null $userAgent The user agent of the request from the user who is attempting to authenticate.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\AuthenticationResponse
     */
    public function authenticateWithCode($clientId, $code, ?string $ipAddress = null, ?string $userAgent = null)
    {
        $path = "user_management/authenticate";
        $params = [
            "client_id" => $clientId,
            "code" => $code,
            "ip_address" => $ipAddress,
            "user_agent" => $userAgent,
            "grant_type" => "authorization_code",
            "client_secret" => WorkOS::getApiKey()
        ];

        $response = Client::request(Client::METHOD_POST, $path, null, $params, true);

        return Resource\AuthenticationResponse::constructFromResponse($response);
    }

    /**
     * Authenticates a user with an unverified email and verifies their email address.
     *
     * @param string $clientId This value can be obtained from the API Keys page in the WorkOS dashboard.
     * @param string $code The authorization value which was passed back as a query parameter in the callback to the Redirect URI.
     * @param string $pendingAuthenticationToken Token returned from a failed authentication attempt due to organization selection being required.
     * @param string|null $ipAddress The IP address of the request from the user who is attempting to authenticate.
     * @param string|null $userAgent The user agent of the request from the user who is attempting to authenticate.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\AuthenticationResponse
     */
    public function authenticateWithEmailVerification($clientId, $code, $pendingAuthenticationToken, ?string $ipAddress = null, ?string $userAgent = null)
    {
        $path = "user_management/authenticate";
        $params = [
            "client_id" => $clientId,
            "code" => $code,
            "pending_authentication_token" => $pendingAuthenticationToken,
            "ip_address" => $ipAddress,
            "user_agent" => $userAgent,
            "grant_type" => "urn:workos:oauth:grant-type:email-verification:code",
            "client_secret" => WorkOS::getApiKey()
        ];

        $response = Client::request(Client::METHOD_POST, $path, null, $params, true);

        return Resource\AuthenticationResponse::constructFromResponse($response);
    }

    /**
     * Authenticate with Magic Auth
     *
     * @param string $clientId This value can be obtained from the API Keys page in the WorkOS dashboard.
     * @param string $code The authorization value which was passed back as a query parameter in the callback to the Redirect URI.
     * @param string $userId The unique ID of the user.
     * @param string|null $ipAddress The IP address of the request from the user who is attempting to authenticate.
     * @param string|null $userAgent The user agent of the request from the user who is attempting to authenticate.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\AuthenticationResponse
     */

    public function authenticateWithMagicAuth(
        $clientId,
        $code,
        $userId,
        ?string $ipAddress = null,
        ?string $userAgent = null
    ) {
        $path = "user_management/authenticate";
        $params = [
            "client_id" => $clientId,
            "code" => $code,
            "user_id" => $userId,
            "ip_address" => $ipAddress,
            "user_agent" => $userAgent,
            "grant_type" => "urn:workos:oauth:grant-type:magic-auth:code",
            "client_secret" => WorkOS::getApiKey()
        ];

        $response = Client::request(Client::METHOD_POST, $path, null, $params, true);

        return Resource\AuthenticationResponse::constructFromResponse($response);
    }

    /**
     * Authenticate with Refresh Token
     * @param string $clientId This value can be obtained from the API Keys page in the WorkOS dashboard.
     * @param string $refreshToken The refresh token used to obtain a new access token
     * @param string|null $ipAddress The IP address of the request from the user who is attempting to authenticate.
     * @param string|null $userAgent The user agent of the request from the user who is attempting to authenticate.
     * @param string|null $organizationId The user agent of the request from the user who is attempting to authenticate.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\AuthenticationResponse
     */
    public function authenticateWithRefreshToken(
        $clientId,
        $refreshToken,
        ?string $ipAddress = null,
        ?string $userAgent = null,
        ?string $organizationId = null
    ) {
        $path = "user_management/authenticate";
        $params = [
            "client_id" => $clientId,
            "refresh_token" => $refreshToken,
            "organization_id" => $organizationId,
            "ip_address" => $ipAddress,
            "user_agent" => $userAgent,
            "grant_type" => "refresh_token",
            "client_secret" => WorkOS::getApiKey()
        ];

        $response = Client::request(Client::METHOD_POST, $path, null, $params, true);

        return Resource\AuthenticationResponse::constructFromResponse($response);
    }

    /**
     * Authenticate with TOTP
     *
     * @param string $clientId This value can be obtained from the API Keys page in the WorkOS dashboard.
     * @param string $pendingAuthenticationToken
     * @param string $authenticationChallengeId
     * @param string $code
     * @param string|null $ipAddress The IP address of the request from the user who is attempting to authenticate.
     * @param string|null $userAgent The user agent of the request from the user who is attempting to authenticate.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\AuthenticationResponse
     */
    public function authenticateWithTotp(
        $clientId,
        $pendingAuthenticationToken,
        $authenticationChallengeId,
        $code,
        ?string $ipAddress = null,
        ?string $userAgent = null
    ) {
        $path = "user_management/authenticate";
        $params = [
            "client_id" => $clientId,
            "pending_authentication_token" => $pendingAuthenticationToken,
            "authentication_challenge_id" => $authenticationChallengeId,
            "code" => $code,
            "ip_address" => $ipAddress,
            "user_agent" => $userAgent,
            "grant_type" => "urn:workos:oauth:grant-type:mfa-totp",
            "client_secret" => WorkOS::getApiKey()
        ];

        $response = Client::request(Client::METHOD_POST, $path, null, $params, true);

        return Resource\AuthenticationResponse::constructFromResponse($response);
    }

    /**
     * Enroll An Authentication Factor.
     *
     * @param string $userId The unique ID of the user.
     * @param string $type The type of MFA factor used to authenticate.
     * @param string|null $totpIssuer Your application or company name, this helps users distinguish between factors in authenticator apps.
     * @param string|null $totpUser Used as the account name in authenticator apps.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\AuthenticationFactorAndChallengeTotp
     */
    public function enrollAuthFactor($userId, $type, ?string $totpIssuer = null, ?string $totpUser = null)
    {
        $path = "user_management/users/{$userId}/auth_factors";

        $params = [
            "type" => $type,
            "totp_user" => $totpUser,
            "totp_issuer" => $totpIssuer
        ];

        $response = Client::request(Client::METHOD_POST, $path, null, $params, true);

        return Resource\AuthenticationFactorAndChallengeTotp::constructFromResponse($response);
    }

    /**
     * List a User's Authentication Factors.
     *
     * @param string $userId The unique ID of the user.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\UserAuthenticationFactorTotp[] $authFactors A list of user's authentication factors
     */
    public function listAuthFactors($userId)
    {
        $path = "user_management/users/{$userId}/auth_factors";

        $response = Client::request(Client::METHOD_GET, $path, null, null, true);

        $authFactors = [];

        foreach ($response["data"] as $responseData) {
            \array_push($authFactors, Resource\UserAuthenticationFactorTotp::constructFromResponse($responseData));
        }

        return $authFactors;
    }

    /**
     * Get an email verification object
     *
     * @param string $emailVerificationId ID of the email verification object
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\EmailVerification
     */
    public function getEmailVerification($emailVerificationId)
    {
        $path = "user_management/email_verification/{$emailVerificationId}";

        $response = Client::request(
            Client::METHOD_GET,
            $path,
            null,
            null,
            true
        );

        return Resource\EmailVerification::constructFromResponse($response);
    }

    /**
     * Create Email Verification Challenge.
     *
     * @param string $userId The unique ID of the User whose email address will be verified.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\UserResponse
     */
    public function sendVerificationEmail($userId)
    {
        $path = "user_management/users/{$userId}/email_verification/send";

        $response = Client::request(Client::METHOD_POST, $path, null, null, true);

        return Resource\UserResponse::constructFromResponse($response);
    }

    /**
     * Complete Email Verification.
     *
     * @param string $userId The unique ID of the user.
     * @param string $code The one-time code emailed to the user.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\UserResponse
     */
    public function verifyEmail($userId, $code)
    {
        $path = "user_management/users/{$userId}/email_verification/confirm";

        $params = [
            "code" => $code
        ];

        $response = Client::request(Client::METHOD_POST, $path, null, $params, true);

        return Resource\UserResponse::constructFromResponse($response);
    }

    /**
     * Get a password reset object
     *
     * @param string $passwordResetId ID of the password reset object
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\PasswordReset
     */
    public function getPasswordReset($passwordResetId)
    {
        $path = "user_management/password_reset/{$passwordResetId}";

        $response = Client::request(
            Client::METHOD_GET,
            $path,
            null,
            null,
            true
        );

        return Resource\PasswordReset::constructFromResponse($response);
    }

    /**
     * Creates a password reset token
     *
     * @param string $email The email address of the user
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\PasswordReset
     */
    public function createPasswordReset(
        $email
    ) {
        $path = "user_management/password_reset";

        $params = [
            "email" => $email
        ];

        $response = Client::request(
            Client::METHOD_POST,
            $path,
            null,
            $params,
            true
        );

        return Resource\PasswordReset::constructFromResponse($response);
    }

    /**
     * @deprecated 4.9.0 Use `createPasswordReset` instead. This method will be removed in a future major version.
     * Create Password Reset Email.
     *
     * @param string $email The email of the user that wishes to reset their password.
     * @param string $passwordResetUrl The URL that will be linked to in the email.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\Response
     */
    public function sendPasswordResetEmail($email, $passwordResetUrl)
    {
        $msg = "'sendPasswordResetEmail' is deprecated. Please use 'createPasswordReset' instead. This method will be removed in a future major version.";

        error_log($msg);

        $path = "user_management/password_reset/send";

        $params = [
            "email" => $email,
            "password_reset_url" => $passwordResetUrl
        ];

        $response = Client::request(Client::METHOD_POST, $path, null, $params, true);

        return $response;
    }

    /**
     * Complete Password Reset.
     *
     * @param string $token The reset token emailed to the user.
     * @param string $newPassword The new password to be set for the user.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\UserResponse
     */
    public function resetPassword($token, $newPassword)
    {
        $path = "user_management/password_reset/confirm";

        $params = [
            "token" => $token,
            "new_password" => $newPassword
        ];

        $response = Client::request(Client::METHOD_POST, $path, null, $params, true);

        return Resource\UserResponse::constructFromResponse($response);
    }

    /**
     * Get a Magic Auth object
     *
     * @param string $magicAuthId ID of the Magic Auth object
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\MagicAuth
     */
    public function getMagicAuth($magicAuthId)
    {
        $path = "user_management/magic_auth/{$magicAuthId}";

        $response = Client::request(
            Client::METHOD_GET,
            $path,
            null,
            null,
            true
        );

        return Resource\MagicAuth::constructFromResponse($response);
    }

    /**
     * Creates a Magic Auth code
     *
     * @param string $email The email address of the user
     * @param string|null $invitationToken The token of an Invitation, if required.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\MagicAuth
     */
    public function createMagicAuth(
        $email,
        ?string $invitationToken = null
    ) {
        $path = "user_management/magic_auth";

        $params = [
            "email" => $email,
            "invitation_token" => $invitationToken
        ];

        $response = Client::request(
            Client::METHOD_POST,
            $path,
            null,
            $params,
            true
        );

        return Resource\MagicAuth::constructFromResponse($response);
    }

    /**
     * @deprecated 4.6.0 Use `createMagicAuth` instead. This method will be removed in a future major version.
     * Creates a one-time Magic Auth code and emails it to the user.
     *
     * @param string $email The email address the one-time code will be sent to.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\Response
     */
    public function sendMagicAuthCode($email)
    {
        $path = "user_management/magic_auth/send";

        $params = [
            "email" => $email,
        ];

        $msg = "'sendMagicAuthCode' is deprecated. Please use 'createMagicAuth' instead. This method will be removed in a future major version.";

        error_log($msg);

        $response = Client::request(
            Client::METHOD_POST,
            $path,
            null,
            $params,
            true
        );

        return $response;
    }

    /**
     * Returns the public key host that is used for verifying access tokens.
     *
     * @param string $clientId This value can be obtained from the API Keys page in the WorkOS dashboard.
     *
     * @throws Exception\UnexpectedValueException
     *
     * @return string
     */
    public function getJwksUrl(string $clientId)
    {
        if (!isset($clientId) || empty($clientId)) {
            throw new Exception\UnexpectedValueException("clientId must not be empty");
        }

        $baseUrl = WorkOS::getApiBaseUrl();

        return "{$baseUrl}sso/jwks/{$clientId}";
    }

    /**
     * Returns the logout URL to end a user's session and redirect to your home page.
     *
     * @param string $sessionId The session ID of the user.
     * @param string|null $return_to The URL to redirect to after the user logs out.
     *
     * @return string
     */
    public function getLogoutUrl(string $sessionId, ?string $return_to = null)
    {
        if (!isset($sessionId) || empty($sessionId)) {
            throw new Exception\UnexpectedValueException("sessionId must not be empty");
        }

        $params = [ "session_id" => $sessionId ];
        if ($return_to) {
            $params["return_to"] = $return_to;
        }

        return Client::generateUrl("user_management/sessions/logout", $params);
    }

    /**
     * List sessions for a user.
     *
     * @param string $userId User ID
     * @param array $options Additional options
     *   - 'limit' (int): Maximum number of records to return (default: 10)
     *   - 'before' (string|null): Session ID to look before
     *   - 'after' (string|null): Session ID to look after
     *   - 'order' (string|null): The order in which to paginate records
     *
     * @return array{?string, ?string, Resource\Session[]}
     *         An array containing before/after cursors and array of Session instances
     * @throws Exception\WorkOSException
     */
    public function listSessions(string $userId, array $options = [])
    {
        $path = "user_management/users/{$userId}/sessions";

        $params = [
            "limit" => $options['limit'] ?? self::DEFAULT_PAGE_SIZE,
            "before" => $options['before'] ?? null,
            "after" => $options['after'] ?? null,
            "order" => $options['order'] ?? null
        ];

        $response = Client::request(
            Client::METHOD_GET,
            $path,
            null,
            $params,
            true
        );

        $sessions = [];
        list($before, $after) = Util\Request::parsePaginationArgs($response);

        foreach ($response["data"] as $responseData) {
            \array_push($sessions, Resource\Session::constructFromResponse($responseData));
        }

        return [$before, $after, $sessions];
    }

    /**
     * Revoke a session.
     *
     * @param string $sessionId Session ID
     *
     * @return Resource\Session The revoked session
     * @throws Exception\WorkOSException
     */
    public function revokeSession(string $sessionId)
    {
        $path = "user_management/sessions/{$sessionId}/revoke";

        $response = Client::request(
            Client::METHOD_POST,
            $path,
            null,
            null,
            true
        );

        return Resource\Session::constructFromResponse($response);
    }

    /**
     * Authenticate with a sealed session cookie.
     *
     * @param string $sealedSession Encrypted session cookie data
     * @param string $cookiePassword Password to decrypt the session
     *
     * @return Resource\SessionAuthenticationSuccessResponse|Resource\SessionAuthenticationFailureResponse
     * @throws Exception\WorkOSException
     */
    public function authenticateWithSessionCookie(
        string $sealedSession,
        string $cookiePassword
    ) {
        if (empty($sealedSession)) {
            return new Resource\SessionAuthenticationFailureResponse(
                Resource\SessionAuthenticationFailureResponse::REASON_NO_SESSION_COOKIE_PROVIDED
            );
        }

        // Tight try/catch for unsealing only
        try {
            $sessionData = $this->getSessionEncryptor()->unseal($sealedSession, $cookiePassword);
        } catch (\Exception $e) {
            return new Resource\SessionAuthenticationFailureResponse(
                Resource\SessionAuthenticationFailureResponse::REASON_ENCRYPTION_ERROR
            );
        }

        if (!isset($sessionData['access_token']) || !isset($sessionData['refresh_token'])) {
            return new Resource\SessionAuthenticationFailureResponse(
                Resource\SessionAuthenticationFailureResponse::REASON_INVALID_SESSION_COOKIE
            );
        }

        // Separate try/catch for HTTP request
        try {
            $path = "user_management/sessions/authenticate";
            $params = [
                "access_token" => $sessionData['access_token'],
                "refresh_token" => $sessionData['refresh_token']
            ];

            $response = Client::request(
                Client::METHOD_POST,
                $path,
                null,
                $params,
                true
            );

            return Resource\SessionAuthenticationSuccessResponse::constructFromResponse($response);
        } catch (Exception\BaseRequestException $e) {
            return new Resource\SessionAuthenticationFailureResponse(
                Resource\SessionAuthenticationFailureResponse::REASON_HTTP_ERROR
            );
        }
    }

    /**
     * Load a sealed session and return a CookieSession instance.
     *
     * @param string $sealedSession Encrypted session cookie data
     * @param string $cookiePassword Password to decrypt the session
     *
     * @return CookieSession
     */
    public function loadSealedSession(string $sealedSession, string $cookiePassword)
    {
        return new CookieSession($this, $sealedSession, $cookiePassword);
    }

    /**
     * Extract and decrypt a session from HTTP cookies.
     *
     * @param string $cookiePassword Password to decrypt the session
     * @param string $cookieName Name of the session cookie (default: 'wos-session')
     *
     * @return CookieSession|null
     */
    public function getSessionFromCookie(
        string $cookiePassword,
        string $cookieName = 'wos-session'
    ) {
        if (!isset($_COOKIE[$cookieName])) {
            return null;
        }

        $sealedSession = $_COOKIE[$cookieName];
        return $this->loadSealedSession($sealedSession, $cookiePassword);
    }
}
