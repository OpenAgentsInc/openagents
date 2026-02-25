<?php

namespace WorkOS;

use WorkOS\Exception;

/**
 * Class SSO.
 * This class facilitates the use of WorkOS SSO.
 */
class SSO
{
    /**
     * Generates an OAuth 2.0 authorization URL used to initiate the SSO flow with WorkOS.
     *
     * @param null|string $domain Domain of the user that will be going through SSO @deprecated 1.5.0 Use $connection or $organization instead.
     * @param null|string $redirectUri URI to direct the user to upon successful completion of SSO
     * @param null|array $state Associative array containing state that will be returned from WorkOS as a json encoded string
     * @param null|string $provider Service provider that handles the identity of the user
     * @param null|string $connection Unique identifier for a WorkOS Connection
     * @param null|string $organization Unique identifier for a WorkOS Organization
     * @param null|string $domainHint Domain hint that will be passed as a parameter to the IdP login page
     * @param null|string $loginHint Username/email hint that will be passed as a parameter to the IdP login page
     *
     * @throws Exception\UnexpectedValueException
     * @throws Exception\ConfigurationException
     *
     * @return string
     */
    public function getAuthorizationUrl(
        $domain,
        $redirectUri,
        $state,
        $provider = null,
        $connection = null,
        ?string $organization = null,
        ?string $domainHint = null,
        ?string $loginHint = null
    ) {
        $authorizationPath = "sso/authorize";

        if (!isset($domain) && !isset($provider) && !isset($connection) && !isset($organization)) {
            $msg = "Either \$domain, \$provider, \$connection, or \$organization is required";

            throw new Exception\UnexpectedValueException($msg);
        }

        if (isset($domain)) {
            $msg = "Domain is being deprecated, please switch to using Connection or Organization ID";

            error_log($msg);
        }

        $params = [
            "client_id" => WorkOS::getClientId(),
            "response_type" => "code"
        ];

        if ($domain) {
            $params["domain"] = $domain;
        }

        if ($redirectUri) {
            $params["redirect_uri"] = $redirectUri;
        }

        if (null !== $state && !empty($state)) {
            $params["state"] = \json_encode($state);
        }

        if ($provider) {
            $params["provider"] = $provider;
        }

        if ($connection) {
            $params["connection"] = $connection;
        }

        if ($organization) {
            $params["organization"] = $organization;
        }

        if ($domainHint) {
            $params["domain_hint"] = $domainHint;
        }

        if ($loginHint) {
            $params["login_hint"] = $loginHint;
        }

        return Client::generateUrl($authorizationPath, $params);
    }

    /**
     * Verify that SSO has been completed successfully and retrieve the identity of the user.
     *
     * @param string $code Code returned by WorkOS on completion of OAuth 2.0 flow
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\ProfileAndToken
     */
    public function getProfileAndToken($code)
    {
        $profilePath = "sso/token";

        $params = [
            "client_id" => WorkOS::getClientId(),
            "client_secret" => WorkOS::getApikey(),
            "code" => $code,
            "grant_type" => "authorization_code"
        ];

        $response = Client::request(Client::METHOD_POST, $profilePath, null, $params);

        return Resource\ProfileAndToken::constructFromResponse($response);
    }

    /**
     * Verify that SSO has been completed successfully and retrieve the identity of the user.
     *
     * @param string $accessToken, the token used to authenticate the API call
     *
     * @throws Exception\GenericException
     *
     * @return Resource\Profile
     */
    public function getProfile($accessToken)
    {
        $getProfilePath = "sso/profile";

        $params = [
            "access_token" => $accessToken
        ];

        $method = Client::METHOD_GET;

        $url = "https://api.workos.com/sso/profile";

        $requestHeaders = ["Authorization: Bearer " . $accessToken];

        list($result) = Client::requestClient()->request(
            $method,
            $url,
            $requestHeaders,
            null
        );

        $decodedResponse = json_decode($result, true);

        $profile = Resource\Profile::constructFromResponse($decodedResponse);

        return $profile->json();
    }

    /**
     * Delete a Connection.
     *
     * @param string $connection Connection ID
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\Response
     */
    public function deleteConnection($connection)
    {
        $connectionPath = "connections/{$connection}";

        $response = Client::request(
            Client::METHOD_DELETE,
            $connectionPath,
            null,
            null,
            true
        );

        return $response;
    }

    /**
     * Get a Connection.
     *
     * @param string $connection Connection ID
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\Connection
     */
    public function getConnection($connection)
    {
        $connectionPath = "connections/{$connection}";

        $response = Client::request(
            Client::METHOD_GET,
            $connectionPath,
            null,
            null,
            true
        );

        return Resource\Connection::constructFromResponse($response);
    }

    public const DEFAULT_PAGE_SIZE = 10;

    /**
     * List Connections.
     *
     * @param null|string $domain Domain of a Connection
     * @param null|string $connectionType Authentication service provider descriptor
     * @param null|string $organizationId Organization ID of the Connection(s)
     * @param int $limit Maximum number of records to return
     * @param null|string $before Connection ID to look before
     * @param null|string $after Connection ID to look after
     * @param Resource\Order $order The Order in which to paginate records
     *
     * @return array{?string, ?string, Resource\Connection[]} An array containing the Directory Connection ID to use as before and after cursor, and an array of Connection instances
     *
     * @throws Exception\WorkOSException
     */
    public function listConnections(
        ?string $domain = null,
        ?string $connectionType = null,
        ?string $organizationId = null,
        $limit = self::DEFAULT_PAGE_SIZE,
        ?string $before = null,
        ?string $after = null,
        ?string $order = null
    ) {
        $connectionsPath = "connections";
        $params = [
            "limit" => $limit,
            "before" => $before,
            "after" => $after,
            "domain" => $domain,
            "connection_type" => $connectionType,
            "organization_id" => $organizationId,
            "order" => $order
        ];

        $response = Client::request(
            Client::METHOD_GET,
            $connectionsPath,
            null,
            $params,
            true
        );

        $connections = [];
        list($before, $after) = Util\Request::parsePaginationArgs($response);
        foreach ($response["data"] as $responseData) {
            \array_push($connections, Resource\Connection::constructFromResponse($responseData));
        }

        return [$before, $after, $connections];
    }
}
