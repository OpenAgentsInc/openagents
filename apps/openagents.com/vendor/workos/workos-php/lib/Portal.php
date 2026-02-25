<?php

namespace WorkOS;

/**
 * Class Portal.
 *
 * This class facilitates the use of the WorkOS Admin Portal.
 */
class Portal
{
    /**
     * Generate a Portal Link scoped to an Organization.
     *
     * @param string $organization An Organization identifier.
     * @param string $intent The intent of the Admin Portal. Possible values are ["audit_logs", "certificate_renewal", "domain_verification", "dsync", "log_streams", "sso"].
     * @param null|string $returnUrl The URL to which WorkOS should send users when they click on
     *      the link to return to your website. (Optional).
     * @param null|string $successUrl The URL to which WorkOS will redirect users to
     *      upon successfully setting up Single Sign On or Directory Sync. (Optional).
     *
     * @throws Exception\UnexpectedValueException
     * @throws Exception\WorkOSException
     *
     * @return Resource\PortalLink
     */
    public function generateLink($organization, $intent, ?string $returnUrl = null, ?string $successUrl = null)
    {
        $validIntents = [
            'audit_logs',
            'certificate_renewal',
            'domain_verification',
            'dsync',
            'log_streams',
            'sso'
        ];

        if (!in_array($intent, $validIntents)) {
            $msg = "Invalid intent. Valid values are: " . implode(", ", $validIntents);
            throw new Exception\UnexpectedValueException($msg);
        }

        $generateLinkPath = "portal/generate_link";
        $params = [
            "organization" => $organization,
            "intent" => $intent,
            "return_url" => $returnUrl,
            "success_url" => $successUrl
        ];

        $response = Client::request(Client::METHOD_POST, $generateLinkPath, null, $params, true);

        return Resource\PortalLink::constructFromResponse($response);
    }
}
