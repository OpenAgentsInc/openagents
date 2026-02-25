<?php

namespace WorkOS;

/**
 * Class Organizations.
 *
 * This class facilitates the use of operations on WorkOS Organizations.
 */
class Organizations
{
    public const DEFAULT_PAGE_SIZE = 10;

    /**
     * List Organizations.
     *
     * @param null|array $domain Filter organizations to only return those that are associated with
     *      the provided domain.
     * @param int $limit Maximum number of records to return
     * @param null|string $before Organization ID to look before
     * @param null|string $after Organization ID to look after
     * @param Resource\Order $order The Order in which to paginate records
     *
     * @return array{?string, ?string, Resource\Organization[]} An array containing the Organization ID to use as before and after cursor, and an array of Organization instances
     *
     * @throws Exception\WorkOSException
     */
    public function listOrganizations(
        ?array $domains = null,
        $limit = self::DEFAULT_PAGE_SIZE,
        ?string $before = null,
        ?string $after = null,
        ?string $order = null
    ) {
        $organizationsPath = "organizations";
        $params = [
            "limit" => $limit,
            "before" => $before,
            "after" => $after,
            "domains" => $domains,
            "order" => $order
        ];

        $response = Client::request(
            Client::METHOD_GET,
            $organizationsPath,
            null,
            $params,
            true
        );

        $organizations = [];
        list($before, $after) = Util\Request::parsePaginationArgs($response);
        foreach ($response["data"] as $responseData) {
            \array_push($organizations, Resource\Organization::constructFromResponse($responseData));
        }

        return [$before, $after, $organizations];
    }

    /**
     * Create Organization.
     *
     * @param string $name The name of the Organization.
     * @param null|array $domains @deprecated 4.5.0 The domains of the Organization. Use domain_data instead.
     * @param null|array $domain_data The domains of the Organization.
     * @param null|boolean $allowProfilesOutsideOrganization @deprecated 4.5.0 If you need to allow sign-ins from
     *      any email domain, contact support@workos.com.
     * @param null|string $idempotencyKey is a unique string that identifies a distinct organization
     * @param null|string $externalId The organization's external id
     * @param null|array<string, string> $metadata The organization's metadata
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\Organization
     */
    public function createOrganization(
        $name,
        ?array $domains = null,
        ?bool $allowProfilesOutsideOrganization = null,
        ?string $idempotencyKey = null,
        ?array $domain_data = null,
        ?string $externalId = null,
        ?array $metadata = null
    ) {
        $idempotencyKey ? $headers = array("Idempotency-Key: $idempotencyKey") : $headers = null;
        $organizationsPath = "organizations";

        $params = ["name" => $name];

        if (isset($domains)) {
            $params["domains"] = $domains;
        }
        if (isset($domain_data)) {
            $params["domain_data"] = $domain_data;
        }
        if (isset($allowProfilesOutsideOrganization)) {
            $params["allow_profiles_outside_organization"] = $allowProfilesOutsideOrganization;
        }
        if (isset($externalId)) {
            $params["external_id"] = $externalId;
        }
        if (isset($metadata)) {
            $params["metadata"] = $metadata;
        }

        $response = Client::request(Client::METHOD_POST, $organizationsPath, $headers, $params, true);

        return Resource\Organization::constructFromResponse($response);
    }

    /**
     * Update Organization.
     *
     * @param string $organization An Organization identifier.
     * @param null|array $domains @deprecated 4.5.0 The domains of the Organization. Use domain_data instead.
     * @param null|array $domain_data The domains of the Organization.
     * @param null|string $name The name of the Organization.
     * @param null|boolean $allowProfilesOutsideOrganization @deprecated 4.5.0 If you need to allow sign-ins from
     *      any email domain, contact support@workos.com.
     * @param null|string $stripeCustomerId The Stripe Customer ID of the Organization.
     * @param null|string $externalId The organization's external id
     * @param null|array<string, string> $metadata The organization's metadata
     *
     * @throws Exception\WorkOSException
     */
    public function updateOrganization(
        $organization,
        ?array $domains = null,
        ?string $name = null,
        ?bool $allowProfilesOutsideOrganization = null,
        ?array $domain_data = null,
        ?string $stripeCustomerId = null,
        ?string $externalId = null,
        ?array $metadata = null
    ) {
        $organizationsPath = "organizations/{$organization}";

        $params = ["name" => $name];

        if (isset($domains)) {
            $params["domains"] = $domains;
        }
        if (isset($domain_data)) {
            $params["domain_data"] = $domain_data;
        }
        if (isset($allowProfilesOutsideOrganization)) {
            $params["allow_profiles_outside_organization"] = $allowProfilesOutsideOrganization;
        }
        if (isset($stripeCustomerId)) {
            $params["stripe_customer_id"] = $stripeCustomerId;
        }
        if (isset($externalId)) {
            $params["external_id"] = $externalId;
        }
        if (isset($metadata)) {
            $params["metadata"] = $metadata;
        }

        $response = Client::request(Client::METHOD_PUT, $organizationsPath, null, $params, true);

        return Resource\Organization::constructFromResponse($response);
    }

    /**
     * Get an Organization
     *
     * @param string $organization WorkOS organization ID
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\Organization
     */
    public function getOrganization($organization)
    {
        $organizationsPath = "organizations/{$organization}";

        $response = Client::request(Client::METHOD_GET, $organizationsPath, null, null, true);

        return Resource\Organization::constructFromResponse($response);
    }

    /**
     * Get an Organization by its external id
     *
     * @param string $externalId external id
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\Organization
     */
    public function getOrganizationByExternalId($externalId)
    {
        $organizationsPath = "organizations/external_id/{$externalId}";

        $response = Client::request(Client::METHOD_GET, $organizationsPath, null, null, true);

        return Resource\Organization::constructFromResponse($response);
    }

    /**
     * Delete an Organization.
     *
     * @param string $organization WorkOS organization ID
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\Response
     */
    public function deleteOrganization($organization)
    {
        $organizationsPath = "organizations/{$organization}";

        $response = Client::request(
            Client::METHOD_DELETE,
            $organizationsPath,
            null,
            null,
            true
        );

        return $response;
    }

    /**
     * List roles for an organization.
     *
     * @param string $organizationId WorkOS organization ID to fetch roles for
     *
     * @throws Exception\WorkOSException
     *
     * @return array{0: Resource\Role[]} An array containing the list of Role instances
     */
    public function listOrganizationRoles($organizationId)
    {
        $organizationRolesPath = "organizations/{$organizationId}/roles";

        $response = Client::request(
            Client::METHOD_GET,
            $organizationRolesPath,
            null,
            null,
            true
        );

        $roles = [];
        foreach ($response["data"] as $responseData) {
            \array_push($roles, Resource\Role::constructFromResponse($responseData));
        }

        return [$roles];
    }

    /**
     * List feature flags for an organization.
     *
     * @param string $organizationId WorkOS organization ID to fetch feature flags for
     * @param int $limit Maximum number of records to return
     * @param null|string $before FeatureFlag ID to look before
     * @param null|string $after FeatureFlag ID to look after
     * @param Resource\Order $order The Order in which to paginate records
     *
     * @throws Exception\WorkOSException
     *
     * @return array{?string, ?string, Resource\FeatureFlag[]} An array containing the FeatureFlag ID to use as before and after cursor, and an array of FeatureFlag instances
     */
    public function listOrganizationFeatureFlags(
        $organizationId,
        $limit = self::DEFAULT_PAGE_SIZE,
        $before = null,
        $after = null,
        $order = null
    ) {
        $featureFlagsPath = "organizations/{$organizationId}/feature-flags";
        $params = [
            "limit" => $limit,
            "before" => $before,
            "after" => $after,
            "order" => $order
        ];

        $response = Client::request(
            Client::METHOD_GET,
            $featureFlagsPath,
            null,
            $params,
            true
        );

        $featureFlags = [];
        list($before, $after) = Util\Request::parsePaginationArgs($response);
        foreach ($response["data"] as $responseData) {
            \array_push($featureFlags, Resource\FeatureFlag::constructFromResponse($responseData));
        }

        return [$before, $after, $featureFlags];
    }
}
