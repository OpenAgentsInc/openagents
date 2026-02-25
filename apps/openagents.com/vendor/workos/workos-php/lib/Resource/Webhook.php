<?php

namespace WorkOS\Resource;

/**
 * Class Webhook.
 *
 * Representation of a webhook resulting from a client ConstructEvent function.
 *
 * @property-read 'user_registration_action_context'|'authentication_action_context' $object The type of webhook event
 *
 * User Registration Action Properties
 * @property-read ?object{
 *     object: 'user_data',
 *     email: string,
 *     first_name: string,
 *     last_name: string
 * } $user_data User information for registration events
 * @property-read ?object{
 *     object: 'invitation',
 *     id: string,
 *     email: string,
 *     expires_at: string,
 *     created_at: string,
 *     updated_at: string,
 *     accepted_at: ?string,
 *     revoked_at: ?string,
 *     organization_id: string,
 *     inviter_user_id: string
 * } $invitation Invitation details for registration events
 *
 * Authentication Action Properties
 * @property-read ?object{
 *     object: 'user',
 *     id: string,
 *     email: string,
 *     first_name: string,
 *     last_name: string,
 *     email_verified: bool,
 *     profile_picture_url: string,
 *     created_at: string,
 *     updated_at: string
 * } $user User information for authentication events
 * @property-read ?string $issuer The authentication issuer
 * @property-read ?object{
 *     object: 'organization',
 *     id: string,
 *     name: string,
 *     allow_profiles_outside_organization: bool,
 *     domains: array<string>,
 *     created_at: string,
 *     updated_at: string
 * } $organization Organization details for authentication events
 * @property-read ?object{
 *     object: 'organization_membership',
 *     id: string,
 *     user_id: string,
 *     organization_id: string,
 *     role: array{slug: string},
 *     status: string,
 *     created_at: string,
 *     updated_at: string
 * } $organization_membership Organization membership details for authentication events
 *
 * Common Properties
 * @property-read string $ip_address IP address of the event
 * @property-read string $user_agent User agent string of the event
 * @property-read string $device_fingerprint Device fingerprint of the event
 */
class Webhook
{
    /**
     * Creates a webhook object from a payload.
     *
     * @param string $payload JSON string containing webhook data
     * @return static
     */
    public static function constructFromPayload($payload)
    {
        $jsonPayload = json_decode($payload);
        $object = (object) $jsonPayload;

        return $object;
    }
}
