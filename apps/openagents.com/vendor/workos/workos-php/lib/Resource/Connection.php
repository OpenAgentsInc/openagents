<?php

namespace WorkOS\Resource;

/**
 * Class Connection.
 */
class Connection extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "connection";

    public const RESOURCE_ATTRIBUTES = [
        "id",
        "domains",
        "state",
        "status",
        "name",
        "connectionType",
        "organizationId",
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "id" => "id",
        "state" => "state",
        "status" => "status",
        "name" => "name",
        "connection_type" => "connectionType",
        "organization_id" => "organizationId",
    ];

    public static function constructFromResponse($response)
    {
        $instance = parent::constructFromResponse($response);

        $rawDomains = $response["domains"];
        $domains = [];
        foreach ($rawDomains as $rawDomain) {
            \array_push($domains, Domain::constructFromResponse($rawDomain));
        }

        $instance->values["domains"] = $domains;

        return $instance;
    }

    public function toArray()
    {
        $connectionArray = parent::toArray();

        $domains = [];
        foreach ($connectionArray["domains"] as $domain) {
            \array_push($domains, $domain->toArray());
        }

        $connectionArray["domains"] = $domains;

        return $connectionArray;
    }
}
