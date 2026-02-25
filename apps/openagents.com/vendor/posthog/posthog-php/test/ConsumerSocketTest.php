<?php

namespace PostHog\Test;

use Exception;
use PHPUnit\Framework\TestCase;
use PostHog\Client;
use RuntimeException;

class ConsumerSocketTest extends TestCase
{
    public function setUp(): void
    {
        date_default_timezone_set("UTC");
    }

    public function testCapture(): void
    {
        $client = new Client(
            "BrpS4SctoaCCsyjlnlun3OzyNJAafdlv__jUWaaJWXg",
            array(
                "consumer" => "socket",
            )
        );
        self::assertTrue(
            $client->capture(
                array(
                    "distinctId" => "some-user",
                    "event" => "Socket PHP Event",
                )
            )
        );
        $client->__destruct();
    }

    public function testIdentify(): void
    {
        $client = new Client(
            "BrpS4SctoaCCsyjlnlun3OzyNJAafdlv__jUWaaJWXg",
            array(
                "consumer" => "socket",
            )
        );
        self::assertTrue(
            $client->identify(
                array(
                    "distinctId" => "Calvin",
                    "properties" => array(
                        "loves_php" => false,
                        "birthday" => time(),
                    ),
                )
            )
        );
        $client->__destruct();
    }

    public function testShortTimeout(): void
    {
        $client = new Client(
            "BrpS4SctoaCCsyjlnlun3OzyNJAafdlv__jUWaaJWXg",
            array(
                "timeout" => 0.01,
                "consumer" => "socket",
            )
        );

        self::assertTrue(
            $client->capture(
                array(
                    "distinctId" => "some-user",
                    "event" => "Socket PHP Event",
                )
            )
        );

        self::assertTrue(
            $client->identify(
                array(
                    "distinctId" => "some-user",
                    "properties" => array(),
                )
            )
        );

        $client->__destruct();
    }

    public function testProductionProblems(): void
    {
        $client = new Client(
            "x",
            array(
                "consumer" => "socket",
                "error_handler" => function () {
                    throw new Exception("Was called");
                },
            )
        );

        // Shouldn't error out without debug on.
        $client->capture(array("user_id" => "some-user", "event" => "Production Problems"));
        $client->__destruct();
        self::assertTrue(true);
    }

    public function testLargeMessage(): void
    {
        $options = array(
            "debug" => true,
            "consumer" => "socket",
        );

        $client = new Client("BrpS4SctoaCCsyjlnlun3OzyNJAafdlv__jUWaaJWXg", $options);

        $big_property = "";

        for ($i = 0; $i < 10000; ++$i) {
            $big_property .= "a";
        }

        self::assertTrue(
            $client->capture(
                array(
                    "distinctId" => "some-user",
                    "event" => "Super Large PHP Event",
                    "properties" => array("big_property" => $big_property),
                )
            )
        );

        $client->__destruct();
    }

    public function testConnectionError(): void
    {
        $this->expectException('RuntimeException');
        $client = new Client(
            "x",
            array(
                "consumer" => "socket",
                "host" => "t.posthog.comcomcom",
                "error_handler" => function ($errno, $errmsg) {
                    throw new RuntimeException($errmsg, $errno);
                },
            )
        );

        $client->capture(array("user_id" => "some-user", "event" => "Event"));
        $client->__destruct();
    }
}
