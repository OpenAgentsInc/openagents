<?php

namespace PostHog\Test;

use PHPUnit\Framework\TestCase;
use PostHog\Client;

class ConsumerForkCurlTest extends TestCase
{
    private $client;

    public function setUp(): void
    {
        date_default_timezone_set("UTC");
        $this->client = new Client(
            "OnMMoZ6YVozrgSBeZ9FpkC0ixH0ycYZn",
            array(
                "consumer" => "fork_curl",
                "debug" => true,
            )
        );
    }

    public function testCapture(): void
    {
        self::assertTrue(
            $this->client->capture(
                array(
                    "distinctId" => "some-user",
                    "event" => "PHP Fork Curl'd\" Event",
                )
            )
        );
    }

    public function testIdentify(): void
    {
        self::assertTrue(
            $this->client->identify(
                array(
                    "distinctId" => "user-id",
                    "properties" => array(
                        "loves_php" => false,
                        "type" => "consumer fork-curl test",
                        "birthday" => time(),
                    ),
                )
            )
        );
    }

    public function testAlias(): void
    {
        self::assertTrue(
            $this->client->alias(
                array(
                    "alias" => "alias-id",
                    "distinctId" => "user-id",
                )
            )
        );
    }
}
