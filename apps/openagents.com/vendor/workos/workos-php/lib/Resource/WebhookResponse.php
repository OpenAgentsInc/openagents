<?php

namespace WorkOS\Resource;

use WorkOS\Webhook;

/**
 * Class WebhookResponse.
 *
 * This class represents the response structure for WorkOS webhook actions.
 */
class WebhookResponse
{
    public const USER_REGISTRATION_ACTION = 'user_registration_action_response';
    public const AUTHENTICATION_ACTION = 'authentication_action_response';
    public const VERDICT_ALLOW = 'Allow';
    public const VERDICT_DENY = 'Deny';

    /**
     * @var string
     */
    private $object;

    /**
     * @var array
     */
    private $payload;

    /**
     * @var string
     */
    private $signature;

    /**
     * Create a new WebhookResponse instance
     *
     * @param string $type Either USER_REGISTRATION_ACTION or AUTHENTICATION_ACTION
     * @param string $secret Webhook secret for signing the response
     * @param string $verdict Either VERDICT_ALLOW or VERDICT_DENY
     * @param string|null $errorMessage Required if verdict is VERDICT_DENY
     * @return self
     * @throws \InvalidArgumentException
     */
    public static function create($type, $secret, $verdict, ?string $errorMessage = null)
    {
        if (!in_array($type, [self::USER_REGISTRATION_ACTION, self::AUTHENTICATION_ACTION])) {
            throw new \InvalidArgumentException('Invalid response type');
        }

        if (empty($secret)) {
            throw new \InvalidArgumentException('Secret is required');
        }

        if (!in_array($verdict, [self::VERDICT_ALLOW, self::VERDICT_DENY])) {
            throw new \InvalidArgumentException('Invalid verdict');
        }

        if ($verdict === self::VERDICT_DENY && empty($errorMessage)) {
            throw new \InvalidArgumentException('Error message is required when verdict is Deny');
        }

        $instance = new self();
        $instance->object = $type;

        $payload = [
            'timestamp' => time() * 1000,
            'verdict' => $verdict
        ];

        if ($verdict === self::VERDICT_DENY) {
            $payload['error_message'] = $errorMessage;
        }

        $instance->payload = $payload;

        $timestamp = $payload['timestamp'];
        $payloadString = json_encode($payload);
        $instance->signature = (new Webhook())->computeSignature($timestamp, $payloadString, $secret);

        return $instance;
    }

    /**
     * Get the response as an array
     *
     * @return array
     */
    public function toArray()
    {
        $response = [
            'object' => $this->object,
            'payload' => $this->payload
        ];

        if ($this->signature) {
            $response['signature'] = $this->signature;
        }

        return $response;
    }

    /**
     * Get the response as a JSON string
     *
     * @return string
     */
    public function toJson()
    {
        return json_encode($this->toArray());
    }
}
