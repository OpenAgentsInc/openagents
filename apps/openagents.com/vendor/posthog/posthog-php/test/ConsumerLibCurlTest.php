<?php

namespace PostHog\Test;

use PHPUnit\Framework\TestCase;
use PostHog\Client;

class ConsumerLibCurlTest extends TestCase
{
    private $client;

    public function setUp(): void
    {
        date_default_timezone_set("UTC");
        $this->client = new Client(
            "BrpS4SctoaCCsyjlnlun3OzyNJAafdlv__jUWaaJWXg",
            [
                "consumer" => "lib_curl",
                "debug" => true,
            ]
        );
    }

    public function testCapture(): void
    {
        self::assertTrue(
            $this->client->capture(
                array(
                    "distinctId" => "lib-curl-capture",
                    "event" => "PHP Lib Curl'd\" Event",
                )
            )
        );
    }

    public function testIdentify(): void
    {
        self::assertTrue(
            $this->client->identify(
                array(
                    "distinctId" => "lib-curl-identify",
                    "properties" => array(
                        "loves_php" => false,
                        "type" => "consumer lib-curl test",
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
                    "alias" => "lib-curl-alias",
                    "distinctId" => "user-id",
                )
            )
        );
    }
}
