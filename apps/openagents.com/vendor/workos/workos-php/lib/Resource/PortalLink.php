<?php

namespace WorkOS\Resource;

/**
 * Class PortalLink.
 */
class PortalLink extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "portal_link";

    public const RESOURCE_ATTRIBUTES = [
        "link"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "link" => "link"
    ];
}
