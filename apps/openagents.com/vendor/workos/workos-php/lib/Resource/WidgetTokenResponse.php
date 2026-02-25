<?php

namespace WorkOS\Resource;

/**
 * Class WidgetTokenResponse.
 *
 * @property string $token
 */
class WidgetTokenResponse extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "widget_token_response";

    public const RESOURCE_ATTRIBUTES = [
        "token"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "token" => "token"
    ];
}
