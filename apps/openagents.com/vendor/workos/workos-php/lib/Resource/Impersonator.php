<?php

namespace WorkOS\Resource;

/**
 * Class Impersonator.
 *
 * @property string $email
 * @property string $reason
 */
class Impersonator extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "impersonator";

    public const RESOURCE_ATTRIBUTES = [
        "email",
        "reason",
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "email" => "email",
        "reason" => "reason",
    ];
}
