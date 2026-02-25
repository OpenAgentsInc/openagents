<?php

namespace WorkOS\Resource;

/**
 * Class AuthenticationFactorSms.
 */
class AuthenticationFactorSms extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "authentication_factor";

    public const RESOURCE_ATTRIBUTES = [
        "object",
        "id",
        "createdAt",
        "updatedAt",
        "type",
        "sms"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "object" => "object",
        "id" => "id",
        "created_at" => "createdAt",
        "updated_at" => "updatedAt",
        "type" => "type",
        "sms" => "sms"
    ];
}
