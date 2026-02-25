<?php

namespace PostHog\Test;

use PHPUnit\Framework\TestCase;
use PostHog\Client;

class ConsumerFileTest extends TestCase
{
    private $client;
    private $filename = "/tmp/posthog.log";

    public function setUp(): void
    {
        date_default_timezone_set("UTC");
        if (file_exists($this->filename())) {
            unlink($this->filename());
        }

        $this->client = new Client(
            "BrpS4SctoaCCsyjlnlun3OzyNJAafdlv__jUWaaJWXg",
            array(
                "consumer" => "file",
                "filename" => $this->filename,
            )
        );
    }

    public function tearDown(): void
    {
        if (file_exists($this->filename)) {
            unlink($this->filename);
        }
    }

    public function testCapture(): void
    {
        self::assertTrue(
            $this->client->capture(
                array(
                    "distinctId" => "some-user",
                    "event" => "File PHP Event - Microtime",
                    "timestamp" => time(),
                )
            )
        );
        $this->checkWritten("capture");
    }

    public function testIdentify(): void
    {
        self::assertTrue(
            $this->client->identify(
                array(
                    "distinctId" => "Calvin",
                    "properties" => array(
                        "loves_php" => false,
                        "type" => "posthog.log",
                        "birthday" => time(),
                    ),
                )
            )
        );
        $this->checkWritten("identify");
    }

    public function testAlias(): void
    {
        self::assertTrue(
            $this->client->alias(
                array(
                    "alias" => "previous-id",
                    "distinctId" => "user-id",
                )
            )
        );

        $this->checkWritten("alias");
    }

    public function testSend(): void
    {
        for ($i = 0; $i < 200; ++$i) {
            $this->client->capture(
                array(
                    "distinctId" => "distinctId",
                    "event" => "event",
                )
            );
        }
        exec("php send.php --apiKey BrpS4SctoaCCsyjlnlun3OzyNJAafdlv__jUWaaJWXg --file /tmp/posthog.log", $output);
        self::assertSame("sent 200 from 200 requests successfully", trim(implode('', $output)));
        self::assertFileDoesNotExist($this->filename());
    }

    public function testProductionProblems(): void
    {
        // Open to a place where we should not have write access.
        $client = new Client(
            "BrpS4SctoaCCsyjlnlun3OzyNJAafdlv__jUWaaJWXg",
            array(
                "consumer" => "file",
                "filename" => "/dev/x/xxxxxxx",
            )
        );

        $captured = $client->capture(array("distinctId" => "some-user", "event" => "my event"));
        self::assertFalse($captured);
    }

    private function checkWritten($type): void
    {
        exec("wc -l " . $this->filename, $output);
        $out = trim($output[0]);
        self::assertSame($out, "1 " . $this->filename);
        $str = file_get_contents($this->filename);
        $json = json_decode(trim($str));
        self::assertSame($type, $json->type);
        unlink($this->filename);
    }

    public function filename(): string
    {
        return '/tmp/posthog.log';
    }
}
