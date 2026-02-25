<?php

namespace WorkOS\Resource;

/**
 * Class Domain.
 */
class Domain extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "connection_domain";

    public const RESOURCE_ATTRIBUTES = [
        "id",
        "domain"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "id" => "id",
        "domain" => "domain"
    ];
}
