<?php

namespace WorkOS;

/**
 * Class Webhook.
 *
 * This class includes functions for users to pass in a webhook header/body and receive
 * the webhook ID, body, and event type if the webhook is valid/secure otherwise an error
 * indicating the issue.
 */
class Webhook
{
    /**
     * Initializes an Event object from a JSON payload
     *
     * @return string|Resource\Webhook
     */
    public function constructEvent($sigHeader, $payload, $secret, $tolerance)
    {
        $eventResult = $this->verifyHeader($sigHeader, $payload, $secret, $tolerance);

        if ($eventResult == 'pass') {
            return Resource\Webhook::constructFromPayload($payload);
        } else {
            return $eventResult;
        }
    }

    /**
     * Verifies the header returned from WorkOS contains a valid timestamp
     * no older than 3 minutes, and computes the signature.
     *
     * @param  string  $sigHeader  WorkOS header containing v1 signature and timestamp
     * @param  string  $payload  Body of the webhook
     * @param  string  $secret  Webhook secret from the WorkOS dashboard
     * @param  int  $tolerance  Number of seconds old the webhook can be before it's invalid
     * @return bool true
     */
    public function verifyHeader($sigHeader, $payload, $secret, $tolerance)
    {
        $timestamp = (int) $this->getTimeStamp($sigHeader);
        $signature = $this->getSignature($sigHeader);

        $currentTime = time();
        $expectedSignature = $this->computeSignature($timestamp, $payload, $secret);

        if (empty($timestamp)) {
            return 'No Timestamp available';
        } elseif (empty($signature)) {
            return 'No signature hash found with expected scheme v1';
        } elseif ($timestamp < $currentTime - $tolerance) {
            return 'Timestamp outside of tolerance';
        } elseif ($signature != $expectedSignature) {
            return 'Constructed signature '.$expectedSignature.'Does not match WorkOS Header Signature '.$signature;
        } else {
            return 'pass';
        }
    }

    /**
     * Splits WorkOS header's two values and pulls out timestamp value and returns it
     *
     * @param  string  $sigHeader  WorkOS header containing v1 signature and timestamp
     * @return $timestamp
     */
    public function getTimeStamp($sigHeader)
    {
        $workosHeadersSplit = explode(',', $sigHeader, 2);
        $timestamp = substr($workosHeadersSplit[0], 2);

        return $timestamp;
    }

    /**
     * Splits WorkOS headers two values and pulls out the signature value and returns it
     *
     * @param  string  $sigHeader  WorkOS header containing v1 signature and timestamp
     * @return string
     */
    public function getSignature($sigHeader)
    {
        $workosHeadersSplit = explode(',', $sigHeader, 2);
        $signature = substr($workosHeadersSplit[1], 4);

        return $signature;
    }

    /**
     * Computes a signature for a webhook payload using the provided timestamp and secret
     *
     * @param  int     $timestamp  Unix timestamp to use in signature
     * @param  string  $payload    The payload to sign
     * @param  string  $secret     Secret key used for signing
     * @return string  The computed HMAC SHA-256 signature
     */
    public function computeSignature($timestamp, $payload, $secret)
    {
        $signedPayload = $timestamp . '.' . $payload;
        return hash_hmac('sha256', $signedPayload, $secret, false);
    }
}
