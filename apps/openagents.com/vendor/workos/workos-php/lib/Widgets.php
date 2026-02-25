<?php

namespace WorkOS;

/**
 * Class Widgets.
 *
 * This class facilitates the use of the WorkOS Widgets API.
 */
class Widgets
{
    /**
     * Generate a widget token scoped to an organization and user.
     *
     * @param string $organization_id An Organization identifier.
     * @param string $user_id An AuthKit user identifier.
     * @param Resource\WidgetScope[] $scopes The scopes to mint the widget token with. Possible values are ["widgets:users-table:manage"].
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\WidgetTokenResponse
     */
    public function getToken($organization_id, $user_id, $scopes)
    {
        $getTokenPath = "widgets/token";
        $params = [
            "organization_id" => $organization_id,
            "user_id" => $user_id,
            "scopes" => $scopes,
        ];

        $response = Client::request(Client::METHOD_POST, $getTokenPath, null, $params, true);

        return Resource\WidgetTokenResponse::constructFromResponse($response);
    }
}
