<?php

namespace WorkOS;

/**
 * Class MFA.
 *
 * This class facilitates the use of WorkOS MFA.
 */
class MFA
{
    /**
     * Enrolls a new Authentication Factor
     *
     * @param string $type - Type of factor to be enrolled (sms or totp)
     * @param null|string $totpIssuer - Name of the Organization
     * @param null|string $totpUser - Email of user
     * @param null|string $phoneNumber - Phone number of user
     *
     * @throws Exception\WorkOSException
     */
    public function enrollFactor(
        $type,
        ?string $totpIssuer = null,
        ?string $totpUser = null,
        ?string $phoneNumber = null
    ) {
        $enrollPath = "auth/factors/enroll";

        if (!isset($type)) {
            $msg = "Incomplete arguments: Need to specify a type of factor";
            throw new Exception\UnexpectedValueException($msg);
        }

        if ($type != "sms" && $type != "totp") {
            $msg = "Type Parameter must either be 'sms' or 'totp'";
            throw new Exception\UnexpectedValueException($msg);
        }

        if ($type == "sms" && !isset($phoneNumber)) {
            $msg = "Incomplete arguments: phoneNumber needs to be specified when using 'sms' as type.";
            throw new Exception\UnexpectedValueException($msg);
        }

        if ($type == "totp" && (!isset($totpIssuer) || !isset($totpUser))) {
            $msg = "Incomplete arguments: totpIssuer and totpUser need to be specified when using 'totp' as type.";
            throw new Exception\UnexpectedValueException($msg);
        }

        $params = [
            "type" => $type,
            "totp_issuer" => $totpIssuer,
            "totp_user" => $totpUser,
            "phone_number" => $phoneNumber
        ];
        $response = Client::request(
            Client::METHOD_POST,
            $enrollPath,
            null,
            $params,
            true
        );

        if ($type == "totp") {
            return Resource\AuthenticationFactorTotp::constructFromResponse($response);
        } elseif ($type == "sms") {
            return Resource\AuthenticationFactorSms::constructFromResponse($response);
        }
    }


    /**
     * Initiates the authentication process (a challenge) for an authentication factor
     *
     * @param string $authenticationFactorId - ID of the authentication factor
     * @param string|null $smsTemplate - Optional parameter to customize the message for sms type factors. Must include "{{code}}" if used.
     *
     * @return Resource\AuthenticationChallengeTotp|Resource\AuthenticationChallengeSms
     */
    public function challengeFactor(
        $authenticationFactorId,
        ?string $smsTemplate = null
    ) {
        if (!isset($authenticationFactorId)) {
            $msg = "Incomplete arguments: 'authentication_factor_id' is a required parameter";
            throw new Exception\UnexpectedValueException($msg);
        }

        $challengePath = "auth/factors/{$authenticationFactorId}/challenge";

        $params = [
        "sms_template" => $smsTemplate
    ];

        $response = Client::request(
            Client::METHOD_POST,
            $challengePath,
            null,
            $params,
            true
        );
        if (isset($response['expires_at'])) {
            return Resource\AuthenticationChallengeSms::constructFromResponse($response);
        } else {
            return Resource\AuthenticationChallengeTotp::constructFromResponse($response);
        }
    }


    /**
     * @deprecated 1.12.0 Use `verifyChallenge` instead. This method will be removed in a future major version.
     * Verifies the one time password provided by the end-user.
     *
     * @param string $authenticationChallengeId - The ID of the authentication challenge that provided the user the verification code.
     * @param string $code - The verification code sent to and provided by the end user.
     */

    public function verifyFactor(
        $authenticationChallengeId,
        $code
    ) {
        if (!isset($authenticationChallengeId) || !isset($code)) {
            $msg = "Incomplete arguments: 'authenticationChallengeId' and 'code' are required parameters";
            throw new Exception\UnexpectedValueException($msg);
        }

        $msg = "'verifyFactor' is deprecated. Please use 'verifyChallenge' instead";

        error_log($msg);

        $response = (new \WorkOS\MFA())
    ->verifyChallenge(
        $authenticationChallengeId,
        $code
    );

        return $response;
    }


    /**
     * Verifies the one time password provided by the end-user.
     *
     * @param string $authenticationChallengeId - The ID of the authentication challenge that provided the user the verification code.
     * @param string $code - The verification code sent to and provided by the end user.
     *
     * @throws Exception\WorkOSException
     */
    public function verifyChallenge(
        $authenticationChallengeId,
        $code
    ) {
        if (!isset($authenticationChallengeId) || !isset($code)) {
            $msg = "Incomplete arguments: 'authenticationChallengeId' and 'code' are required parameters";
            throw new Exception\UnexpectedValueException($msg);
        }

        $verifyPath = "auth/challenges/{$authenticationChallengeId}/verify";

        $params = [
        "code" => $code
    ];

        $response = Client::request(
            Client::METHOD_POST,
            $verifyPath,
            null,
            $params,
            true
        );

        return Resource\VerificationChallenge::constructFromResponse($response);
    }


    /**
     * Returns a Factor.
     *
     * @param string $authenticationFactorId - WorkOS Factor ID
     *
     * @throws Exception\WorkOSException
     */
    public function getFactor($authenticationFactorId)
    {
        $getFactorPath = "auth/factors/{$authenticationFactorId}";

        $response = Client::request(
            Client::METHOD_GET,
            $getFactorPath,
            null,
            null,
            true
        );

        return Resource\AuthenticationFactorTotp::constructFromResponse($response);
    }


    /**
     * Deletes a Factor.
     *
     * @param string $authenticationFactorId - WorkOS Factor ID
     *
     * @return Resource\Response
     *
     * @throws Exception\WorkOSException
     */
    public function deleteFactor($authenticationFactorId)
    {
        $deleteFactorPath = "auth/factors/{$authenticationFactorId}";

        $response = Client::request(
            Client::METHOD_DELETE,
            $deleteFactorPath,
            null,
            null,
            true
        );

        return $response;
    }
}
