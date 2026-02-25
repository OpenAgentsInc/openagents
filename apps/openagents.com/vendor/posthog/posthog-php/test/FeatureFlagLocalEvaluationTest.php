<?php
// phpcs:ignoreFile
namespace PostHog\Test;

// comment out below to print all logs instead of failing tests
require_once 'test/error_log_mock.php';

use Exception;
use PHPUnit\Framework\TestCase;
use SlopeIt\ClockMock\ClockMock;
use PostHog\FeatureFlag;
use PostHog\Client;
use PostHog\PostHog;
use PostHog\Test\Assets\MockedResponses;
use PostHog\InconclusiveMatchException;
use PostHog\SizeLimitedHash;

class FeatureFlagLocalEvaluationTest extends TestCase
{
    protected const FAKE_API_KEY = "random_key";

    protected Client $client;

    protected MockedHttpClient $http_client;

    public function setUp(): void
    {
        date_default_timezone_set("UTC");

        // Reset the errorMessages array before each test
        global $errorMessages;
        $errorMessages = [];
    }

    public function checkEmptyErrorLogs(): void
    {
        global $errorMessages;
        $this->assertTrue(empty($errorMessages), "Error logs are not empty: " . implode("\n", $errorMessages));
    }

    public function testMatchPropertyEquals(): void
    {
        $prop = [
            "key" => "key",
            "value" => "value",
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "value",
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => "value2",
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => "",
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => null,
        ]));

        self::expectException(InconclusiveMatchException::class);
        FeatureFlag::matchProperty($prop, [
            "key2" => "value2",
        ]);

        $prop = [
            "key" => "key",
            "value" => "value",
            "operator" => "exact"
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "value",
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => "value2",
        ]));

        $prop = [
            "key" => "key",
            "value" => ["value1", "value2", "value3"],
            "operator" => "exact"
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "value1",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "value2",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "value3",
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => "value4",
        ]));

        self::expectException(InconclusiveMatchException::class);
        FeatureFlag::matchProperty($prop, [
            "key2" => "value",
        ]);
    }

    public function testMatchPropertyNotIn(): void
    {
        $prop = [
            "key" => "key",
            "value" => "value",
            "operator" => "is_not"
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "value2",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => null,
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "",
        ]));

        $prop = [
            "key" => "key",
            "value" => ["value1", "value2", "value3"],
            "operator" => "is_not"
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "value4",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "value5",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "value6",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => null,
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => "value2",
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => "value3",
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => "value1",
        ]));

        self::expectException(InconclusiveMatchException::class);
        FeatureFlag::matchProperty($prop, [
            "key2" => "value",
        ]);
    }

    public function testMatchPropertyIsSet(): void
    {
        $prop = [
            "key" => "key",
            "value" => "is_set",
            "operator" => "is_set"
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "value",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "value2",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => null,
        ]));

        self::expectException(InconclusiveMatchException::class);
        FeatureFlag::matchProperty($prop, [
            "key2" => "value",
        ]);
    }

    public function testMatchPropertyContains(): void
    {
        $prop = [
            "key" => "key",
            "value" => "valUe",
            "operator" => "icontains"
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "value2",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "value3",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "value4",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "343tfvalue5",
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => "Alakazam",
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => 123,
        ]));

        $prop = [
            "key" => "key",
            "value" => 3,
            "operator" => "icontains"
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "3",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => 323,
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "val3",
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => "three",
        ]));
    }

    public function testMatchPropertyRegex(): void
    {
        $prop = [
            "key" => "key",
            "value" => ".com",
            "operator" => "regex"
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "value.com",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "value2.com",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => ".com343tfvalue5",
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => "Alakazam",
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => 123,
        ]));


        $prop = [
            "key" => "key",
            "value" => "3",
            "operator" => "regex"
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "3",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => 323,
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "val3",
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => "three",
        ]));

        $prop = [
            "key" => "key",
            "value" => "?*",
            "operator" => "regex"
        ];

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => "value",
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => "value2",
        ]));

        $prop = [
            "key" => "key",
            "value" => "4",
            "operator" => "regex"
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "4",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => 4,
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => "value",
        ]));
    }

    public function testMatchPropertyMathOperators(): void
    {
        $prop = [
            "key" => "key",
            "value" => 1,
            "operator" => "gt"
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => 2,
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => 3,
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => 0,
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => -1,
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "23",
        ]));

        $prop = [
            "key" => "key",
            "value" => 1,
            "operator" => "lt"
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => 0,
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => -1,
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => -3,
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => 1,
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => "1",
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => "3",
        ]));

        $prop = [
            "key" => "key",
            "value" => 1,
            "operator" => "gte"
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => 1,
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => 2,
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => 0,
        ]));

        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => -1,
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "3",
        ]));

        $prop = [
            "key" => "key",
            "value" => 43,
            "operator" => "lte"
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => 0,
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => 43,
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => 42,
        ]));


        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => 44,
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "1",
        ]));
        self::assertFalse(FeatureFlag::matchProperty($prop, [
            "key" => "50",
        ]));

        self::assertTrue(FeatureFlag::matchProperty($prop, [
            "key" => "3",
        ]));

        $prop_e = [
            "key" => "key",
            "value" => "30",
            "operator" => "lt"
        ];
        self::assertTrue(FeatureFlag::matchProperty($prop_e, [
            "key" => "29",
        ]));
        # depending on the type of override, we adjust type comparison
        self::assertTrue(FeatureFlag::matchProperty($prop_e, [
            "key" => "100",
        ]));
        self::assertFalse(FeatureFlag::matchProperty($prop_e, [
            "key" => 100,
        ]));

        $prop_f = [
            "key" => "key",
            "value" => "123aloha",
            "operator" => "gt"
        ];
        self::assertFalse(FeatureFlag::matchProperty($prop_f, [
            "key" => "123",
        ]));
        self::assertFalse(FeatureFlag::matchProperty($prop_f, [
            "key" => 122,
        ]));

        # this turns into a string comparison
        self::assertTrue(FeatureFlag::matchProperty($prop_f, [
            "key" => 129,
        ]));


    }

    public function testMatchPropertyDateOperators(): void
    {
        // is date before
        $prop_a = [
            "key" => "key",
            "value" => "2022-05-01",
            "operator" => "is_date_before"
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop_a, [
            "key" => "2022-03-01",
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop_a, [
            "key" => "2022-04-30",
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop_a, [
            "key" => new \DateTime('2022-04-30'),
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop_a, [
            "key" => new \DateTime('2022-04-30 01:02:03'),
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop_a, [
            "key" => new \DateTime('2022-04-30T00:00:00+02:00'),
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop_a, [
            "key" => new \DateTime('2022-04-30'),
        ]));
        self::assertFalse(FeatureFlag::matchProperty($prop_a, [
            "key" => "2022-05-30",
        ]));

        // is date after
        // is date after
        // const property_b = { key: 'key', value: '2022-05-01', operator: 'is_date_after' }
        // expect(matchProperty(property_b, { key: '2022-05-02' })).toBe(true)
        // expect(matchProperty(property_b, { key: '2022-05-30' })).toBe(true)
        // expect(matchProperty(property_b, { key: new Date(2022, 4, 30) })).toBe(true)
        // expect(matchProperty(property_b, { key: new Date('2022-05-30') })).toBe(true)
        // expect(matchProperty(property_b, { key: '2022-04-30' })).toBe(false)
        $prop_b = [
            "key" => "key",
            "value" => "2022-05-01",
            "operator" => "is_date_after"
        ];
        self::assertTrue(FeatureFlag::matchProperty($prop_b, [
            "key" => "2022-05-02",
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop_b, [
            "key" => "2022-05-30",
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop_b, [
            "key" => new \DateTime('2022-05-30'),
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop_b, [
            "key" => new \DateTime('2022-05-30 01:02:03'),
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop_b, [
            "key" => new \DateTime('2022-05-30T00:00:00+02:00'),
        ]));
        self::assertFalse(FeatureFlag::matchProperty($prop_b, [
            "key" => "2022-04-30",
        ]));

        // can't be an invalid number or invalid string
        self::expectException(Exception::class);
        FeatureFlag::matchProperty($prop_a, [
            "key" => "abcdef",
        ]);
        self::expectException(InconclusiveMatchException::class);
        FeatureFlag::matchProperty($prop_a, [
            "key" => "62802180000012345",
        ]);

        // // invalid flag property
        // const property_c = { key: 'key', value: 'abcd123', operator: 'is_date_before' }
        $prop_c = [
            "key" => "key",
            "value" => "abcd123",
            "operator" => "is_date_before"
        ];
        self::expectException(InconclusiveMatchException::class);
        FeatureFlag::matchProperty($prop_c, [
            "key" => "2022-05-30",
        ]);

        // // Timezone
        $prop_d = [
            "key" => "key",
            "value" => "2022-04-05 12:34:12 +01:00",
            "operator" => "is_date_before"
        ];
        self::assertFalse(FeatureFlag::matchProperty($prop_d, [
            "key" => "2022-05-30",
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop_d, [
            "key" => "2022-03-30",
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop_d, [
            "key" => "2022-04-05 12:34:11+01:00",
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop_d, [
            "key" => "2022-04-05 11:34:11 +00:00",
        ]));
        self::assertFalse(FeatureFlag::matchProperty($prop_d, [
            "key" => "2022-04-05 11:34:13 +00:00",
        ]));
    }

    public function testMatchPropertyRelativeDateOperators(): void
    {
        ClockMock::executeAtFrozenDateTime(new \DateTime('2022-05-01'), function () {

            $prop_a = [
                "key" => "key",
                "value" => "-6h",
                "operator" => "is_date_before"
            ];
            self::assertTrue(FeatureFlag::matchProperty($prop_a, [
                "key" => "2022-03-01",
            ]));
            self::assertTrue(FeatureFlag::matchProperty($prop_a, [
                "key" => "2022-04-30",
            ]));

            self::assertTrue(FeatureFlag::matchProperty($prop_a, [
                "key" => new \DateTime('2022-04-30 01:02:03'),
            ]));
            // false because date comparison, instead of datetime, so reduces to same date
            self::assertFalse(FeatureFlag::matchProperty($prop_a, [
                "key" => new \DateTime('2022-04-30 19:02:03'),
            ]));
            self::assertTrue(FeatureFlag::matchProperty($prop_a, [
                "key" => new \DateTime('2022-04-30T01:02:03+02:00'),
            ]));
            self::assertFalse(FeatureFlag::matchProperty($prop_a, [
                "key" => new \DateTime('2022-04-30T20:02:03+02:00'),
            ]));
            self::assertTrue(FeatureFlag::matchProperty($prop_a, [
                "key" => new \DateTime('2022-04-30T19:59:03+02:00'),
            ]));
            self::assertTrue(FeatureFlag::matchProperty($prop_a, [
                "key" => new \DateTime('2022-04-30'),
            ]));
            self::assertFalse(FeatureFlag::matchProperty($prop_a, [
                "key" => "2022-05-30",
            ]));

            // // # can't be an invalid string
            try {
                FeatureFlag::matchProperty($prop_a, [
                    "key" => "abcdef",
                ]);
            } catch (Exception $exception) {
                self::assertStringContainsString("Failed to parse time string (abcdef) at position 0 (a): The timezone could not be found in the database", $exception->getMessage());
            }

            $prop_b = [
                "key" => "key",
                "value" => "1h",
                "operator" => "is_date_after"
            ];
            self::assertTrue(FeatureFlag::matchProperty($prop_b, [
                "key" => "2022-05-02",
            ]));
            self::assertTrue(FeatureFlag::matchProperty($prop_b, [
                "key" => "2022-05-30",
            ]));
            self::assertTrue(FeatureFlag::matchProperty($prop_b, [
                "key" => new \DateTime('2022-05-30'),
            ]));
            self::assertTrue(FeatureFlag::matchProperty($prop_b, [
                "key" => new \DateTime('2022-05-30 01:02:03'),
            ]));
            self::assertFalse(FeatureFlag::matchProperty($prop_b, [
                "key" => new \DateTime('2022-04-30 01:02:03'),
            ]));

            $prop_c = [
                "key" => "key",
                "value" => 1234,
                "operator" => "is_date_after"
            ];

            try {
                FeatureFlag::matchProperty($prop_c, [
                    "key" => "2022-05-30",
                ]);
            } catch (InconclusiveMatchException $exception) {
                self::assertStringContainsString("The date provided 1234 must be a string or date object", $exception->getMessage());
            }

            try {
                FeatureFlag::matchProperty($prop_c, [
                    "key" => 1,
                ]);
            } catch (InconclusiveMatchException $exception) {
                self::assertStringContainsString("The date provided 1234 must be a string or date object", $exception->getMessage());
            }

            // # Try all possible relative dates
            $prop_e = [
                "key" => "key",
                "value" => "1h",
                "operator" => "is_date_before"
            ];
            self::assertFalse(FeatureFlag::matchProperty($prop_e, [
                "key" => "2022-05-01 00:00:00",
            ]));
            self::assertTrue(FeatureFlag::matchProperty($prop_e, [
                "key" => "2022-04-30 22:00:00",
            ]));

            $prop_f = [
                "key" => "key",
                "value" => "1d",
                "operator" => "is_date_before"
            ];
            self::assertTrue(FeatureFlag::matchProperty($prop_f, [
                "key" => "2022-04-29 23:59:00",
            ]));
            self::assertFalse(FeatureFlag::matchProperty($prop_f, [
                "key" => "2022-04-30 00:00:01",
            ]));

            $prop_g = [
                "key" => "key",
                "value" => "1w",
                "operator" => "is_date_before"
            ];
            self::assertTrue(FeatureFlag::matchProperty($prop_g, [
                "key" => "2022-04-23 00:00:00",
            ]));
            self::assertFalse(FeatureFlag::matchProperty($prop_g, [
                "key" => "2022-04-24 00:00:00",
            ]));
            self::assertFalse(FeatureFlag::matchProperty($prop_g, [
                "key" => "2022-04-24 00:00:01",
            ]));

            $prop_h = [
                "key" => "key",
                "value" => "1m",
                "operator" => "is_date_before"
            ];
            self::assertTrue(FeatureFlag::matchProperty($prop_h, [
                "key" => "2022-03-01 00:00:00",
            ]));
            self::assertFalse(FeatureFlag::matchProperty($prop_h, [
                "key" => "2022-04-05 00:00:00",
            ]));

            $prop_i = [
                "key" => "key",
                "value" => "1y",
                "operator" => "is_date_before"
            ];
            self::assertTrue(FeatureFlag::matchProperty($prop_i, [
                "key" => "2021-04-28 00:00:00",
            ]));
            self::assertFalse(FeatureFlag::matchProperty($prop_i, [
                "key" => "2021-05-01 00:00:01",
            ]));

            $prop_j = [
                "key" => "key",
                "value" => "122h",
                "operator" => "is_date_after"
            ];
            self::assertTrue(FeatureFlag::matchProperty($prop_j, [
                "key" => "2022-05-01 00:00:00",
            ]));
            self::assertFalse(FeatureFlag::matchProperty($prop_j, [
                "key" => "2022-04-23 01:00:00",
            ]));

            $prop_k = [
                "key" => "key",
                "value" => "2d",
                "operator" => "is_date_after"
            ];
            self::assertTrue(FeatureFlag::matchProperty($prop_k, [
                "key" => "2022-05-01 00:00:00",
            ]));
            self::assertTrue(FeatureFlag::matchProperty($prop_k, [
                "key" => "2022-04-29 00:00:01",
            ]));
            self::assertFalse(FeatureFlag::matchProperty($prop_k, [
                "key" => "2022-04-29 00:00:00",
            ]));

            $prop_l = [
                "key" => "key",
                "value" => "-02w",
                "operator" => "is_date_after"
            ];
            self::assertTrue(FeatureFlag::matchProperty($prop_l, [
                "key" => "2022-05-01 00:00:00",
            ]));
            self::assertFalse(FeatureFlag::matchProperty($prop_l, [
                "key" => "2022-04-16 00:00:00",
            ]));

            $prop_m = [
                "key" => "key",
                "value" => "1m",
                "operator" => "is_date_after"
            ];
            self::assertTrue(FeatureFlag::matchProperty($prop_m, [
                "key" => "2022-04-01 00:00:01",
            ]));

            self::assertFalse(FeatureFlag::matchProperty($prop_m, [
                "key" => "2022-04-01 00:00:00",
            ]));

            $prop_n = [
                "key" => "key",
                "value" => "-1y",
                "operator" => "is_date_after"
            ];
            self::assertTrue(FeatureFlag::matchProperty($prop_n, [
                "key" => "2022-05-01 00:00:00",
            ]));
            self::assertTrue(FeatureFlag::matchProperty($prop_n, [
                "key" => "2021-05-01 00:00:01",
            ]));
            self::assertFalse(FeatureFlag::matchProperty($prop_n, [
                "key" => "2021-05-01 00:00:00",
            ]));
            self::assertFalse(FeatureFlag::matchProperty($prop_n, [
                "key" => "2021-04-30 00:00:00",
            ]));
            self::assertFalse(FeatureFlag::matchProperty($prop_n, [
                "key" => "2021-03-01 12:13:00",
            ]));
        });
    }

    public function testMatchPropertyWithNones(): void
    {
        $prop_a = [
            "key" => "key",
            "value" => "null",
            "operator" => "is_not"
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop_a, [
            "key" => null,
        ]));
        self::assertFalse(FeatureFlag::matchProperty($prop_a, [
            "key" => "null",
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop_a, [
            "key" => "nul",
        ]));

        $prop_b = [
            "key" => "key",
            "value" => "null",
            "operator" => "is_set"
        ];

        self::assertTrue(FeatureFlag::matchProperty($prop_b, [
            "key" => null,
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop_b, [
            "key" => "null",
        ]));

        $prop_c = [
            "key" => "key",
            "value" => "null",
            "operator" => "regex"
        ];
        self::assertFalse(FeatureFlag::matchProperty($prop_c, [
            "key" => null,
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop_c, [
            "key" => "null",
        ]));

        $prop_e = [
            "key" => "key",
            "value" => "null",
            "operator" => "gt"
        ];
        self::assertFalse(FeatureFlag::matchProperty($prop_e, [
            "key" => null,
        ]));
        self::assertFalse(FeatureFlag::matchProperty($prop_e, [
            "key" => "null",
        ]));

        $prop_f = [
            "key" => "key",
            "value" => "null",
            "operator" => "lt"
        ];
        self::assertTrue(FeatureFlag::matchProperty($prop_f, [
            "key" => null,
        ]));
        self::assertFalse(FeatureFlag::matchProperty($prop_f, [
            "key" => "null",
        ]));

        $prop_g = [
            "key" => "key",
            "value" => "null",
            "operator" => "gte"
        ];
        self::assertFalse(FeatureFlag::matchProperty($prop_g, [
            "key" => null,
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop_g, [
            "key" => "null",
        ]));

        $prop_h = [
            "key" => "key",
            "value" => "null",
            "operator" => "lte"
        ];
        self::assertTrue(FeatureFlag::matchProperty($prop_h, [
            "key" => null,
        ]));
        self::assertTrue(FeatureFlag::matchProperty($prop_h, [
            "key" => "null",
        ]));

    }

    public function testRelativeDateParsingInvalidInput()
    {
        self::assertNull(FeatureFlag::relativeDateParseForFeatureFlagMatching('1'));
        self::assertNull(FeatureFlag::relativeDateParseForFeatureFlagMatching('1x'));
        self::assertNull(FeatureFlag::relativeDateParseForFeatureFlagMatching('1.2y'));
        self::assertNull(FeatureFlag::relativeDateParseForFeatureFlagMatching('1z'));
        self::assertNull(FeatureFlag::relativeDateParseForFeatureFlagMatching('1s'));
        self::assertNull(FeatureFlag::relativeDateParseForFeatureFlagMatching('123344000m'));
        self::assertNull(FeatureFlag::relativeDateParseForFeatureFlagMatching('bazinga'));
        self::assertNull(FeatureFlag::relativeDateParseForFeatureFlagMatching('000bello'));
        self::assertNull(FeatureFlag::relativeDateParseForFeatureFlagMatching('000hello'));

        self::assertNotNull(FeatureFlag::relativeDateParseForFeatureFlagMatching('000h'));
        self::assertNotNull(FeatureFlag::relativeDateParseForFeatureFlagMatching('1000h'));

    }

    public function testRelativeDateParsingOverflow()
    {
        self::assertNull(FeatureFlag::relativeDateParseForFeatureFlagMatching('1000000h'));
        self::assertNull(FeatureFlag::relativeDateParseForFeatureFlagMatching('100000000000000000y'));
    }

    public function testRelativeDateParsingHours()
    {

        ClockMock::executeAtFrozenDateTime(new \DateTime('2020-01-01T12:01:20Z'), function () {
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('1h'), new \DateTime('2020-01-01T11:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('2h'), new \DateTime('2020-01-01T10:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('24h'), new \DateTime('2019-12-31T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('30h'), new \DateTime('2019-12-31T06:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('48h'), new \DateTime('2019-12-30T12:01:20Z'));

            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('24h'), FeatureFlag::relativeDateParseForFeatureFlagMatching('1d'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('48h'), FeatureFlag::relativeDateParseForFeatureFlagMatching('2d'));
        });
    }

    public function testRelativeDateParsingDays()
    {
        ClockMock::executeAtFrozenDateTime(new \DateTime('2020-01-01T12:01:20Z'), function () {
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('1d'), new \DateTime('2019-12-31T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('2d'), new \DateTime('2019-12-30T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('7d'), new \DateTime('2019-12-25T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('14d'), new \DateTime('2019-12-18T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('30d'), new \DateTime('2019-12-02T12:01:20Z'));

            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('7d'), FeatureFlag::relativeDateParseForFeatureFlagMatching('1w'));
        });
    }

    public function testRelativeDateParsingWeeks()
    {
        ClockMock::executeAtFrozenDateTime(new \DateTime('2020-01-01T12:01:20Z'), function () {
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('1w'), new \DateTime('2019-12-25T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('2w'), new \DateTime('2019-12-18T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('4w'), new \DateTime('2019-12-04T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('8w'), new \DateTime('2019-11-06T12:01:20Z'));

            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('1m'), new \DateTime('2019-12-01T12:01:20Z'));
            self::assertNotEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('4w'), FeatureFlag::relativeDateParseForFeatureFlagMatching('1m'));
        });
    }

    public function testRelativeDateParsingMonths()
    {
        ClockMock::executeAtFrozenDateTime(new \DateTime('2020-01-01T12:01:20Z'), function () {
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('1m'), new \DateTime('2019-12-01T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('2m'), new \DateTime('2019-11-01T12:01:20Z'));

            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('4m'), new \DateTime('2019-09-01T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('5m'), new \DateTime('2019-08-01T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('6m'), new \DateTime('2019-07-01T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('8m'), new \DateTime('2019-05-01T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('10m'), new \DateTime('2019-03-01T12:01:20Z'));

            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('24m'), new \DateTime('2018-01-01T12:01:20Z'));

            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('1y'), new \DateTime('2019-01-01T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('12m'), FeatureFlag::relativeDateParseForFeatureFlagMatching('1y'));
        });

        ClockMock::executeAtFrozenDateTime(new \DateTime('2020-04-03T00:00:00Z'), function () {
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('1m'), new \DateTime('2020-03-03T00:00:00Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('2m'), new \DateTime('2020-02-03T00:00:00Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('4m'), new \DateTime('2019-12-03T00:00:00Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('8m'), new \DateTime('2019-08-03T00:00:00Z'));

            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('1y'), new \DateTime('2019-04-03T00:00:00Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('12m'), FeatureFlag::relativeDateParseForFeatureFlagMatching('1y'));

        });
    }

    public function testRelativeDateParsingYears()
    {
        ClockMock::executeAtFrozenDateTime(new \DateTime('2020-01-01T12:01:20Z'), function () {

            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('1y'), new \DateTime('2019-01-01T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('2y'), new \DateTime('2018-01-01T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('4y'), new \DateTime('2016-01-01T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('8y'), new \DateTime('2012-01-01T12:01:20Z'));

            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('1y'), new \DateTime('2019-01-01T12:01:20Z'));
            self::assertEquals(FeatureFlag::relativeDateParseForFeatureFlagMatching('12m'), FeatureFlag::relativeDateParseForFeatureFlagMatching('1y'));
        });
    }






    public function testFlagPersonProperties()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_REQUEST);

        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );

        PostHog::init(null, null, $this->client);

        $this->assertTrue(PostHog::getFeatureFlag('person-flag', 'some-distinct-id', [], ["region" => "USA"]));
        $this->assertFalse(PostHog::getFeatureFlag('person-flag', 'some-distinct-id-2', [], ["region" => "Canada"]));

        $this->checkEmptyErrorLogs();
    }

    public function testFlagPersonBooleanProperties()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_BOOLEAN_REQUEST);

        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );

        PostHog::init(null, null, $this->client);

        $this->assertTrue(PostHog::getFeatureFlag('person-flag', 'some-distinct-id', [], ["region" => "true", "region_array" => "true"], [], true, false));

        PostHog::flush();
        $this->assertEquals(
            $this->http_client->calls,
            array(
                0 => array(
                    "path" => "/api/feature_flag/local_evaluation?send_cohorts&token=random_key",
                    "payload" => null,
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION, 1 => 'Authorization: Bearer test'),
                    "requestOptions" => array("includeEtag" => true),
                ),
                // no decide or capture calls
            )
        );

        $this->checkEmptyErrorLogs();

        // reset calls
        $this->http_client->calls = array();

        $this->assertTrue(PostHog::getFeatureFlag('person-flag', 'some-distinct-id', [], ["region" => "true", "region_array" => true], [], true, false));
        $this->assertTrue(PostHog::getFeatureFlag('person-flag', 'some-distinct-id', [], ["region" => true, "region_array" => true], [], true, false));
        $this->assertTrue(PostHog::getFeatureFlag('person-flag', 'some-distinct-id', [], ["region" => true, "region_array" => "true"], [], true, false));
        $this->assertFalse(PostHog::getFeatureFlag('person-flag', 'some-distinct-id', [], ["region" => 1, "region_array" => "1"], [], true, false));
        $this->assertFalse(PostHog::getFeatureFlag('person-flag', 'some-distinct-id', [], ["region" => true, "region_array" => "1"], [], true, false));
        $this->assertFalse(PostHog::getFeatureFlag('person-flag', 'some-distinct-id', [], ["region" => "1", "region_array" => "true"], [], true, false));

        $this->assertEquals(
            $this->http_client->calls,
            array()
                // no decide or capture calls
        );

        $this->assertTrue(PostHog::getFeatureFlag('person-flag-with-boolean', 'some-distinct-id', [], ["region" => "true", "region_array" => true], [], true, false));
        $this->assertTrue(PostHog::getFeatureFlag('person-flag-with-boolean', 'some-distinct-id', [], ["region" => "true", "region_array" => true], [], true, false));
        $this->assertTrue(PostHog::getFeatureFlag('person-flag-with-boolean', 'some-distinct-id', [], ["region" => true, "region_array" => true], [], true, false));
        $this->assertTrue(PostHog::getFeatureFlag('person-flag-with-boolean', 'some-distinct-id', [], ["region" => true, "region_array" => "true"], [], true, false));
        $this->assertFalse(PostHog::getFeatureFlag('person-flag-with-boolean', 'some-distinct-id', [], ["region" => true, "region_array" => "false"], [], true, false));
        $this->assertFalse(PostHog::getFeatureFlag('person-flag-with-boolean', 'some-distinct-id', [], ["region" => false, "region_array" => "true"], [], true, false));

        $this->assertEquals(
            $this->http_client->calls,
            array()
                // no decide or capture calls
        );

        $this->assertTrue(PostHog::getFeatureFlag('person-flag-with-boolean-icontains', 'some-distinct-id', [], ["region" => "true", "region_array" => true], [], true, false));
        $this->assertTrue(PostHog::getFeatureFlag('person-flag-with-boolean-icontains', 'some-distinct-id', [], ["region" => true, "region_array" => true], [], true, false));
        $this->assertFalse(PostHog::getFeatureFlag('person-flag-with-boolean-icontains', 'some-distinct-id', [], ["region" => false, "region_array" => "true"], [], true, false));

        $this->assertEquals(
            $this->http_client->calls,
            array()
                // no decide or capture calls
        );
    }

    public function testFlagGroupProperties()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_GROUP_PROPERTIES_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $this->assertFalse(PostHog::getFeatureFlag('group-flag', 'some-distinct-1', [], [], ["company" => ["name" => "Project Name 1"]]));
        $this->assertFalse(PostHog::getFeatureFlag('group-flag', 'some-distinct-2', [], [], ["company" => ["name" => "Project Name 2"]]));
        $this->assertTrue(PostHog::getFeatureFlag('group-flag', 'some-distinct-id', ["company" => "amazon_without_rollout"], [], ["company" => ["name" => "Project Name 1"]]));
        $this->assertFalse(PostHog::getFeatureFlag('group-flag', 'some-distinct-id', ["company" => "amazon"], [], ["company" => ["name" => "Project Name 1"]]));
        $this->assertFalse(PostHog::getFeatureFlag('group-flag', 'some-distinct-id', ["company" => "amazon_without_rollout"], [], ["company" => ["name" => "Project Name 2"]]));
        $this->assertEquals(PostHog::getFeatureFlag('group-flag', 'some-distinct-id', ["company" => "amazon"], [], ["company" => []]), 'decide-fallback-value');
    }

    public function testFlagComplexDefinition()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_COMPLEX_FLAG_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $this->assertTrue(PostHog::getFeatureFlag('complex-flag', 'some-distinct-id', [], ["region" => "USA", "name" => "Aloha"], []));
        $this->assertTrue(PostHog::getFeatureFlag('complex-flag', 'some-distinct-within-roll', [], ["region" => "USA", "email" => "a@b.com"], []));
        $this->assertEquals(PostHog::getFeatureFlag('complex-flag', 'some-distinct-within-rollout', [], ["region" => "USA", "email" => "a@b.com"], []), 'decide-fallback-value');
        $this->assertEquals(PostHog::getFeatureFlag('complex-flag', 'some-distinct-within-rollout', [], ["doesnt_matter" => "1"], []), 'decide-fallback-value');
        $this->assertEquals(PostHog::getFeatureFlag('complex-flag', 'some-distinct-id', [], ["region" => "USA"], []), 'decide-fallback-value');
        $this->assertFalse(PostHog::getFeatureFlag('complex-flag', 'some-distinct-within-rollout', [], ["region" => "USA", "email" => "a@b.com", "name" => "X", "doesnt_matter" => "1"], []));
    }

    public function testFlagFallbackToDecide()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::FALLBACK_TO_FLAGS_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $this->assertEquals(PostHog::getFeatureFlag('feature-1', 'some-distinct'), 'decide-fallback-value');
        $this->assertEquals(PostHog::getFeatureFlag('feature-2', 'some-distinct'), 'decide-fallback-value');

        $this->checkEmptyErrorLogs();
    }

    public function testFlagFallbackToDecideWithFalseFlag()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::FALLBACK_TO_FLAGS_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $this->assertEquals(PostHog::getFeatureFlag('unknown-flag???', 'some-distinct'), null);
        $this->assertEquals(PostHog::getFeatureFlag('false-flag', 'some-distinct'), null);

        $this->checkEmptyErrorLogs();
    }

    public function testFeatureFlagDefaultsComeIntoPlayOnlyWhenDecideErrorsOut()
    {
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            null,
            null
        );
        PostHog::init(null, null, $this->client);
        $this->assertEquals(PostHog::getFeatureFlag('simple-flag', 'distinct-id'), null);
    }


    public function testFlagExperienceContinuityNotEvaluatedLocally()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::EXPERIENCE_CONITNUITY_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $this->assertEquals(PostHog::getFeatureFlag('beta-feature', 'distinct-id', [], [], []), 'decide-fallback-value');
    }

    public function testGetAllFlagsWithFallback()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::MULTIPLE_FLAGS_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $flags = PostHog::getAllFlags('distinct-id');

        $this->assertEquals($flags["variant-1"], "variant-1");
        $this->assertEquals($flags["variant-2"], false);
        $this->assertEquals($flags["variant-3"], "variant-3");
    }

    public function testGetAllFlagsWithFallbackEmptyLocalFlags()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: []);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $flags = PostHog::getAllFlags('distinct-id');

        $this->assertEquals($flags["variant-1"], "variant-1");
        $this->assertEquals($flags["variant-3"], "variant-3");
    }

    public function testGetAllFlagsWithNoFallback()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::MULTIPLE_FLAGS_LOCAL_EVALUATE_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $flags = PostHog::getAllFlags('distinct-id');

        $this->assertEquals($flags["variant-1"], true);
        $this->assertEquals($flags["variant-2"], false);
    }

    public function testLoadFeatureFlags()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_GROUP_PROPERTIES_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $this->assertEquals(count($this->client->featureFlags), 1);
        $this->assertEquals($this->client->featureFlags[0]["key"], "group-flag");

        $this->assertEquals($this->client->groupTypeMapping, [
            "0" => "company",
            "1" => "project"
        ]);
    }

    public function testLoadFeatureFlagsWrongKey()
    {
        self::expectException(Exception::class);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            null,
            self::FAKE_API_KEY
        );
        PostHog::init(null, null, $this->client);
    }

    public function testSimpleFlag()
    {
        ClockMock::executeAtFrozenDateTime(new \DateTime('2022-05-01'), function () {

            $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_SIMPLE_REQUEST);
            $this->client = new Client(
                self::FAKE_API_KEY,
                [
                    "debug" => true,
                ],
                $this->http_client,
                "test"
            );
            PostHog::init(null, null, $this->client);

            $this->assertTrue(PostHog::getFeatureFlag('simple-flag', 'some-distinct-id'));

            PostHog::flush();

            $this->assertEquals(
                $this->http_client->calls,
                array(
                    0 => array(
                        "path" => "/api/feature_flag/local_evaluation?send_cohorts&token=random_key",
                        "payload" => null,
                        "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION, 1 => 'Authorization: Bearer test'),
                        "requestOptions" => array("includeEtag" => true),
                    ),
                    1 => array(
                        "path" => "/batch/",
                        'payload' => '{"batch":[{"properties":{"$feature\/simple-flag":true,"$active_feature_flags":["simple-flag"],"$feature_flag":"simple-flag","$feature_flag_response":true,"$lib":"posthog-php","$lib_version":"' . PostHog::VERSION . '","$lib_consumer":"LibCurl","$groups":[]},"distinct_id":"some-distinct-id","event":"$feature_flag_called","$groups":[],"library":"posthog-php","library_version":"' . PostHog::VERSION . '","library_consumer":"LibCurl","groups":[],"timestamp":"2022-05-01T00:00:00+00:00","type":"capture"}],"api_key":"random_key"}',
                        "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                        "requestOptions" => array('shouldVerify' => true),
                    ),
                )
            );
        });
    }

    public function testFeatureFlagsDontFallbackToDecideWhenOnlyLocalEvaluationIsTrue()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::FALLBACK_TO_FLAGS_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        # beta-feature should fallback to decide because property type is unknown,
        # but doesn't because only_evaluate_locally is true
        $this->assertEquals(PostHog::getFeatureFlag(
            'beta-feature',
            'some-distinct-id',
            array(),
            array(),
            array(),
            true,
            false
        ), null);

        $this->assertEquals(PostHog::isFeatureEnabled(
            'beta-feature',
            'some-distinct-id',
            array(),
            array(),
            array(),
            true,
            false
        ), null);

        # beta-feature2 should fallback to decide because region property not given with call
        # but doesn't because only_evaluate_locally is true
        $this->assertEquals(PostHog::getFeatureFlag(
            'beta-feature2',
            'some-distinct-id',
            array(),
            array(),
            array(),
            true,
            false
        ), null);

        $this->assertEquals(PostHog::isFeatureEnabled(
            'beta-feature2',
            'some-distinct-id',
            array(),
            array(),
            array(),
            true,
            false
        ), false);
    }

    public function testComputingInactiveFlagLocally()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_WITH_INACTIVE_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $flags = PostHog::getAllFlags('distinct-id');

        $this->assertEquals($flags, [
            "enabled-flag" => true,
            "disabled-flag" => false
        ]);

        $this->assertEquals(
            $this->http_client->calls,
            array(
                0 => array(
                    "path" => "/api/feature_flag/local_evaluation?send_cohorts&token=random_key",
                    "payload" => null,
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION, 1 => 'Authorization: Bearer test'),
                    "requestOptions" => array("includeEtag" => true),
                ),
                // no decide or capture calls
            )
        );
    }

    public function testFeatureFlagsLocalEvaluationForCohorts()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_WITH_COHORTS_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $feature_flag_match = PostHog::getFeatureFlag(
            "beta-feature",
            "some-distinct-id",
            [],
            ["region" => "UK"]
        );

        $this->assertEquals($feature_flag_match, false);
        $this->assertEquals(
            $this->http_client->calls,
            array(
                0 => array(
                    "path" => "/api/feature_flag/local_evaluation?send_cohorts&token=random_key",
                    "payload" => null,
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION, 1 => 'Authorization: Bearer test'),
                    "requestOptions" => array("includeEtag" => true),
                ),
            )
        );

        PostHog::flush();
        // reset calls
        $this->http_client->calls = array();

        $feature_flag_match = PostHog::getFeatureFlag(
            "beta-feature",
            "some-distinct-id",
            [],
            ["region" => "USA", "nation" => "UK"]
        );

        $this->assertEquals($feature_flag_match, true);
        $this->assertEquals(
            $this->http_client->calls,
            // no decide calls
            array()
        );

        PostHog::flush();

        // reset calls
        $this->http_client->calls = array();

        $feature_flag_match = PostHog::getFeatureFlag(
            "beta-feature",
            "some-distinct-id",
            [],
            ["region" => "USA", "other" => "thing"]
        );

        $this->assertEquals($feature_flag_match, true);
        $this->assertEquals(
            $this->http_client->calls,
            // no decide calls
            array()
        );
    }

    public function testFeatureFlagsLocalEvaluationForNegatedCohorts()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_FOR_NEGATED_COHORTS_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $feature_flag_match = PostHog::getFeatureFlag(
            "beta-feature",
            "some-distinct-id",
            [],
            ["region" => "UK"]
        );

        $this->assertEquals($feature_flag_match, false);
        $this->assertEquals(
            $this->http_client->calls,
            array(
                0 => array(
                    "path" => "/api/feature_flag/local_evaluation?send_cohorts&token=random_key",
                    "payload" => null,
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION, 1 => 'Authorization: Bearer test'),
                    "requestOptions" => array("includeEtag" => true),
                ),
            )
        );

        PostHog::flush();
        // reset calls
        $this->http_client->calls = array();

        $feature_flag_match = PostHog::getFeatureFlag(
            "beta-feature",
            "some-distinct-id",
            [],
            ["region" => "USA", "nation" => "UK"]
        );

        // even though 'other' property is not present, the cohort should still match since it's an OR condition
        $this->assertEquals($feature_flag_match, true);
        $this->assertEquals(
            $this->http_client->calls,
            // no decide calls
            array()
        );

        PostHog::flush();
        // reset calls
        $this->http_client->calls = array();

        $feature_flag_match = PostHog::getFeatureFlag(
            "beta-feature",
            "some-distinct-id",
            [],
            ["region" => "USA", "other" => "thing"]
        );
        # since 'other' is negated, we return False. Since 'nation' is not present, we can't tell whether the flag should be true or false, so go to decide
        $this->assertEquals($feature_flag_match, 'decide-fallback-value');
        $this->assertEquals(
            $this->http_client->calls,
            array(
                0 => array(
                    "path" => "/flags/?v=2",
                    'payload' => '{"api_key":"random_key","distinct_id":"some-distinct-id","person_properties":{"distinct_id":"some-distinct-id","region":"USA","other":"thing"}}',
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                    "requestOptions" => array("timeout" => 3000, "shouldRetry" => false),
                ),
            )
        );

        PostHog::flush();
        // reset calls
        $this->http_client->calls = array();

        $feature_flag_match = PostHog::getFeatureFlag(
            "beta-feature",
            "some-distinct-id",
            [],
            ["region" => "USA", "other" => "thing2"]
        );

        $this->assertEquals($feature_flag_match, true);
        $this->assertEquals(
            $this->http_client->calls,
            // no decide calls
            array()
        );
    }

    public function testComputingFlagWithoutRolloutLocally()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_WITH_NO_ROLLOUT_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $flags = PostHog::getAllFlags('distinct-id');

        $this->assertEquals($flags, [
            "enabled-flag" => true,
        ]);
    }

    public function testFlagWithVariantOverrides()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_VARIANT_OVERRIDES_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $this->assertEquals(PostHog::getFeatureFlag('beta-feature', 'test_id', [], ["email" => "test@posthog.com"]), "second-variant");
        $this->assertEquals(PostHog::getFeatureFlag('beta-feature', 'example_id'), "first-variant");
    }

    public function testFlagWithClashingVariantOverrides()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_CLASHING_VARIANT_OVERRIDES_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $this->assertEquals(PostHog::getFeatureFlag('beta-feature', 'test_id', [], ["email" => "test@posthog.com"]), "second-variant");
        $this->assertEquals(PostHog::getFeatureFlag('beta-feature', 'example_id', [], ["email" => "test@posthog.com"]), "second-variant");
        $this->assertEquals(PostHog::getFeatureFlag('beta-feature', 'example_id'), "first-variant");
    }

    public function testFlagWithInvalidVariantOverrides()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_INVALID_VARIANT_OVERRIDES_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $this->assertEquals(PostHog::getFeatureFlag('beta-feature', 'test_id', [], ["email" => "test@posthog.com"]), "third-variant");
        $this->assertEquals(PostHog::getFeatureFlag('beta-feature', 'example_id'), "second-variant");
    }

    public function testConditionsEvaluatedInOrder()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_CONDITIONS_ORDER_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        // VIP users now match the first condition (100% rollout) instead of their specific variant override
        // because conditions are evaluated in order
        $result = PostHog::getFeatureFlag('test-flag', 'vip_user', [], ["email" => "user@vip.com"]);
        $this->assertTrue(in_array($result, ['control', 'test'])); // Should get one of the regular variants, not vip-variant
    }

    public function testEventCalled()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_SIMPLE_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );

        $this->client->distinctIdsFeatureFlagsReported = new SizeLimitedHash(1);
        PostHog::init(null, null, $this->client);

        PostHog::getFeatureFlag('simple-flag', 'some-distinct-id');
        $this->assertEquals($this->client->distinctIdsFeatureFlagsReported->count(), 1);

        PostHog::getFeatureFlag('simple-flag', 'some-distinct-id2');
        $this->assertEquals($this->client->distinctIdsFeatureFlagsReported->count(), 1);
    }

    public function testFlagConsistency()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::SIMPLE_PARTIAL_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $result = [
            false,
            true,
            true,
            false,
            true,
            false,
            false,
            true,
            false,
            true,
            false,
            true,
            true,
            false,
            true,
            false,
            false,
            false,
            true,
            true,
            false,
            true,
            false,
            false,
            true,
            false,
            true,
            true,
            false,
            false,
            false,
            true,
            true,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            false,
            true,
            true,
            false,
            true,
            true,
            false,
            false,
            false,
            true,
            true,
            false,
            false,
            false,
            false,
            true,
            false,
            true,
            false,
            true,
            false,
            true,
            true,
            false,
            true,
            false,
            true,
            false,
            true,
            true,
            false,
            false,
            true,
            false,
            false,
            true,
            false,
            true,
            false,
            false,
            true,
            false,
            false,
            false,
            true,
            true,
            false,
            true,
            true,
            false,
            true,
            true,
            true,
            true,
            true,
            false,
            true,
            true,
            false,
            false,
            true,
            true,
            true,
            true,
            false,
            false,
            true,
            false,
            true,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            false,
            true,
            true,
            true,
            false,
            false,
            true,
            false,
            true,
            false,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            true,
            true,
            false,
            false,
            true,
            false,
            false,
            true,
            true,
            false,
            false,
            true,
            false,
            true,
            false,
            true,
            true,
            true,
            false,
            false,
            false,
            true,
            false,
            false,
            false,
            false,
            true,
            true,
            false,
            true,
            true,
            false,
            true,
            false,
            true,
            true,
            false,
            true,
            false,
            true,
            true,
            true,
            false,
            true,
            false,
            false,
            true,
            true,
            false,
            true,
            false,
            true,
            true,
            false,
            false,
            true,
            true,
            true,
            true,
            false,
            true,
            true,
            false,
            false,
            true,
            false,
            true,
            false,
            false,
            true,
            true,
            false,
            true,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            true,
            true,
            false,
            false,
            true,
            false,
            true,
            false,
            false,
            false,
            true,
            false,
            true,
            false,
            false,
            false,
            true,
            false,
            false,
            true,
            false,
            true,
            true,
            false,
            false,
            false,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            true,
            true,
            false,
            true,
            false,
            true,
            true,
            false,
            true,
            false,
            true,
            false,
            false,
            false,
            true,
            true,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            true,
            true,
            true,
            false,
            false,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            true,
            true,
            true,
            true,
            false,
            true,
            true,
            true,
            false,
            false,
            true,
            false,
            true,
            false,
            false,
            true,
            true,
            true,
            false,
            true,
            false,
            false,
            false,
            true,
            true,
            false,
            true,
            false,
            true,
            false,
            true,
            true,
            true,
            true,
            true,
            false,
            false,
            true,
            false,
            true,
            false,
            true,
            true,
            true,
            false,
            true,
            false,
            true,
            true,
            false,
            true,
            true,
            true,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            true,
            false,
            false,
            true,
            true,
            false,
            false,
            false,
            true,
            false,
            true,
            true,
            true,
            true,
            false,
            false,
            false,
            false,
            true,
            true,
            false,
            false,
            true,
            true,
            false,
            true,
            true,
            true,
            true,
            false,
            true,
            true,
            true,
            false,
            false,
            true,
            true,
            false,
            false,
            true,
            false,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            true,
            true,
            false,
            false,
            true,
            false,
            false,
            true,
            false,
            true,
            false,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            true,
            true,
            true,
            false,
            false,
            false,
            true,
            false,
            true,
            false,
            false,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            true,
            false,
            true,
            true,
            true,
            false,
            false,
            false,
            true,
            true,
            true,
            false,
            true,
            false,
            true,
            true,
            false,
            false,
            false,
            true,
            false,
            false,
            false,
            false,
            true,
            false,
            true,
            false,
            true,
            true,
            false,
            true,
            false,
            false,
            false,
            true,
            false,
            false,
            true,
            true,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            false,
            true,
            true,
            false,
            false,
            true,
            false,
            false,
            true,
            true,
            true,
            false,
            false,
            false,
            true,
            false,
            false,
            false,
            false,
            true,
            false,
            true,
            false,
            false,
            false,
            true,
            false,
            true,
            true,
            false,
            true,
            false,
            true,
            false,
            true,
            false,
            false,
            true,
            false,
            false,
            true,
            false,
            true,
            false,
            true,
            false,
            true,
            false,
            false,
            true,
            true,
            true,
            true,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            false,
            true,
            false,
            false,
            true,
            true,
            false,
            false,
            false,
            false,
            true,
            true,
            true,
            false,
            false,
            true,
            false,
            false,
            true,
            true,
            true,
            true,
            false,
            false,
            false,
            true,
            false,
            false,
            false,
            true,
            false,
            false,
            true,
            true,
            true,
            true,
            false,
            false,
            true,
            true,
            false,
            true,
            false,
            true,
            false,
            false,
            true,
            true,
            false,
            true,
            true,
            true,
            true,
            false,
            false,
            true,
            false,
            false,
            true,
            true,
            false,
            true,
            false,
            true,
            false,
            false,
            true,
            false,
            false,
            false,
            false,
            true,
            true,
            true,
            false,
            true,
            false,
            false,
            true,
            false,
            false,
            true,
            false,
            false,
            false,
            false,
            true,
            false,
            true,
            false,
            true,
            true,
            false,
            false,
            true,
            false,
            true,
            true,
            true,
            false,
            false,
            false,
            false,
            true,
            true,
            false,
            true,
            false,
            false,
            false,
            true,
            false,
            false,
            false,
            false,
            true,
            true,
            true,
            false,
            false,
            false,
            true,
            true,
            true,
            true,
            false,
            true,
            true,
            false,
            true,
            true,
            true,
            false,
            true,
            false,
            false,
            true,
            false,
            true,
            true,
            true,
            true,
            false,
            true,
            false,
            true,
            false,
            true,
            false,
            false,
            true,
            true,
            false,
            false,
            true,
            false,
            true,
            false,
            false,
            false,
            false,
            true,
            false,
            true,
            false,
            false,
            false,
            true,
            true,
            true,
            false,
            false,
            false,
            true,
            false,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            true,
            false,
            false,
            true,
            true,
            false,
            true,
            true,
            true,
            true,
            false,
            false,
            true,
            false,
            false,
            true,
            false,
            true,
            false,
            true,
            true,
            false,
            false,
            false,
            true,
            false,
            true,
            true,
            false,
            false,
            false,
            true,
            false,
            true,
            false,
            true,
            true,
            false,
            true,
            false,
            false,
            true,
            false,
            false,
            false,
            true,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            false,
            true,
            true,
            true,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            true,
            true,
            true,
            false,
            false,
            true,
            true,
            false,
            true,
            true,
            false,
            true,
            false,
            true,
            false,
            false,
            false,
            true,
            false,
            false,
            true,
            false,
            false,
            true,
            true,
            true,
            true,
            false,
            false,
            true,
            false,
            true,
            true,
            false,
            false,
            true,
            false,
            false,
            true,
            true,
            false,
            true,
            false,
            false,
            true,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            true,
            true,
            false,
            true,
            true,
            true,
            false,
            false,
            false,
            false,
            true,
            true,
            true,
            true,
            false,
            true,
            true,
            false,
            true,
            false,
            true,
            false,
            true,
            false,
            false,
            false,
            false,
            true,
            true,
            true,
            true,
            false,
            false,
            true,
            false,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            true,
            false,
            true,
            true,
            false,
            false,
            true,
            true,
            true,
            true,
            false,
            false,
            true,
            false,
            true,
            true,
            false,
            false,
            true,
            true,
            true,
            false,
            true,
            false,
            false,
            true,
            true,
            false,
            false,
            false,
            true,
            false,
            false,
            true,
            false,
            false,
            false,
            true,
            true,
            true,
            true,
            false,
            true,
            false,
            true,
            false,
            true,
            false,
            true,
            false,
            false,
            true,
            false,
            false,
            true,
            false,
            true,
            true,
        ];
        foreach (range(0, 999) as $number) {
            $testResult = PostHog::getFeatureFlag('simple-flag', sprintf('distinct_id_%s', $number));
            $this->assertEquals($testResult, $result[$number]);
        }
    }

    public function testMultivariateFlagConsistency()
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::MULTIVARIATE_REQUEST);

        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        $result = [
            "second-variant",
            "second-variant",
            "first-variant",
            false,
            false,
            "second-variant",
            "first-variant",
            false,
            false,
            false,
            "first-variant",
            "third-variant",
            false,
            "first-variant",
            "second-variant",
            "first-variant",
            false,
            false,
            "fourth-variant",
            "first-variant",
            false,
            "third-variant",
            false,
            false,
            false,
            "first-variant",
            "first-variant",
            "first-variant",
            "first-variant",
            "first-variant",
            "first-variant",
            "third-variant",
            false,
            "third-variant",
            "second-variant",
            "first-variant",
            false,
            "third-variant",
            false,
            false,
            "first-variant",
            "second-variant",
            false,
            "first-variant",
            "first-variant",
            "second-variant",
            false,
            "first-variant",
            false,
            false,
            "first-variant",
            "first-variant",
            "first-variant",
            "second-variant",
            "first-variant",
            false,
            "second-variant",
            "second-variant",
            "third-variant",
            "second-variant",
            "first-variant",
            false,
            "first-variant",
            "second-variant",
            "fourth-variant",
            false,
            "first-variant",
            "first-variant",
            "first-variant",
            false,
            "first-variant",
            "second-variant",
            false,
            "third-variant",
            false,
            false,
            false,
            false,
            false,
            false,
            "first-variant",
            "fifth-variant",
            false,
            "second-variant",
            "first-variant",
            "second-variant",
            false,
            "third-variant",
            "third-variant",
            false,
            false,
            false,
            false,
            "third-variant",
            false,
            false,
            "first-variant",
            "first-variant",
            false,
            "third-variant",
            "third-variant",
            false,
            "third-variant",
            "second-variant",
            "third-variant",
            false,
            false,
            "second-variant",
            "first-variant",
            false,
            false,
            "first-variant",
            false,
            false,
            false,
            false,
            "first-variant",
            "first-variant",
            "first-variant",
            false,
            false,
            false,
            "first-variant",
            "first-variant",
            false,
            "first-variant",
            "first-variant",
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            "first-variant",
            "first-variant",
            "first-variant",
            "first-variant",
            "second-variant",
            "first-variant",
            "first-variant",
            "first-variant",
            "second-variant",
            false,
            "second-variant",
            "first-variant",
            "second-variant",
            "first-variant",
            false,
            "second-variant",
            "second-variant",
            false,
            "first-variant",
            false,
            false,
            false,
            "third-variant",
            "first-variant",
            false,
            false,
            "first-variant",
            false,
            false,
            false,
            false,
            "first-variant",
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            "first-variant",
            "first-variant",
            "third-variant",
            "first-variant",
            "first-variant",
            false,
            false,
            "first-variant",
            false,
            false,
            "fifth-variant",
            "second-variant",
            false,
            "second-variant",
            false,
            "first-variant",
            "third-variant",
            "first-variant",
            "fifth-variant",
            "third-variant",
            false,
            false,
            "fourth-variant",
            false,
            false,
            false,
            false,
            "third-variant",
            false,
            false,
            "third-variant",
            false,
            "first-variant",
            "second-variant",
            "second-variant",
            "second-variant",
            false,
            "first-variant",
            "third-variant",
            "first-variant",
            "first-variant",
            false,
            false,
            false,
            false,
            false,
            "first-variant",
            "first-variant",
            "first-variant",
            "second-variant",
            false,
            false,
            false,
            "second-variant",
            false,
            false,
            "first-variant",
            false,
            "first-variant",
            false,
            false,
            "first-variant",
            "first-variant",
            "first-variant",
            "first-variant",
            "third-variant",
            "first-variant",
            "third-variant",
            "first-variant",
            "first-variant",
            "second-variant",
            "third-variant",
            "third-variant",
            false,
            "second-variant",
            "first-variant",
            false,
            "second-variant",
            "first-variant",
            false,
            "first-variant",
            false,
            false,
            "first-variant",
            "fifth-variant",
            "first-variant",
            false,
            false,
            false,
            false,
            "first-variant",
            "first-variant",
            "second-variant",
            false,
            "second-variant",
            "third-variant",
            "third-variant",
            false,
            "first-variant",
            "third-variant",
            false,
            false,
            "first-variant",
            false,
            "third-variant",
            "first-variant",
            false,
            "third-variant",
            "first-variant",
            "first-variant",
            false,
            "first-variant",
            "second-variant",
            "second-variant",
            "first-variant",
            false,
            false,
            false,
            "second-variant",
            false,
            false,
            "first-variant",
            "first-variant",
            false,
            "third-variant",
            false,
            "first-variant",
            false,
            "third-variant",
            false,
            "third-variant",
            "second-variant",
            "first-variant",
            false,
            false,
            "first-variant",
            "third-variant",
            "first-variant",
            "second-variant",
            "fifth-variant",
            false,
            false,
            "first-variant",
            false,
            false,
            false,
            "third-variant",
            false,
            "second-variant",
            "first-variant",
            false,
            false,
            false,
            false,
            "third-variant",
            false,
            false,
            "third-variant",
            false,
            false,
            "first-variant",
            "third-variant",
            false,
            false,
            "first-variant",
            false,
            false,
            "fourth-variant",
            "fourth-variant",
            "third-variant",
            "second-variant",
            "first-variant",
            "third-variant",
            "fifth-variant",
            false,
            "first-variant",
            "fifth-variant",
            false,
            "first-variant",
            "first-variant",
            "first-variant",
            false,
            false,
            false,
            "second-variant",
            "fifth-variant",
            "second-variant",
            "first-variant",
            "first-variant",
            "second-variant",
            false,
            false,
            "third-variant",
            false,
            "second-variant",
            "fifth-variant",
            false,
            "third-variant",
            "first-variant",
            false,
            false,
            "fourth-variant",
            false,
            false,
            "second-variant",
            false,
            false,
            "first-variant",
            "fourth-variant",
            "first-variant",
            "second-variant",
            false,
            false,
            false,
            "first-variant",
            "third-variant",
            "third-variant",
            false,
            "first-variant",
            "first-variant",
            "first-variant",
            false,
            "first-variant",
            false,
            "first-variant",
            "third-variant",
            "third-variant",
            false,
            false,
            "first-variant",
            false,
            false,
            "second-variant",
            "second-variant",
            "first-variant",
            "first-variant",
            "first-variant",
            false,
            "fifth-variant",
            "first-variant",
            false,
            false,
            false,
            "second-variant",
            "third-variant",
            "first-variant",
            "fourth-variant",
            "first-variant",
            "third-variant",
            false,
            "first-variant",
            "first-variant",
            false,
            "third-variant",
            "first-variant",
            "first-variant",
            "third-variant",
            false,
            "fourth-variant",
            "fifth-variant",
            "first-variant",
            "first-variant",
            false,
            false,
            false,
            "first-variant",
            "first-variant",
            "first-variant",
            false,
            "first-variant",
            "first-variant",
            "second-variant",
            "first-variant",
            false,
            "first-variant",
            "second-variant",
            "first-variant",
            false,
            "first-variant",
            "second-variant",
            false,
            "first-variant",
            "first-variant",
            false,
            "first-variant",
            false,
            "first-variant",
            false,
            "first-variant",
            false,
            false,
            false,
            "third-variant",
            "third-variant",
            "first-variant",
            false,
            false,
            "second-variant",
            "third-variant",
            "first-variant",
            "first-variant",
            false,
            false,
            false,
            "second-variant",
            "first-variant",
            false,
            "first-variant",
            "third-variant",
            false,
            "first-variant",
            false,
            false,
            false,
            "first-variant",
            "third-variant",
            "third-variant",
            false,
            false,
            false,
            false,
            "third-variant",
            "fourth-variant",
            "fourth-variant",
            "first-variant",
            "second-variant",
            false,
            "first-variant",
            false,
            "second-variant",
            "first-variant",
            "third-variant",
            false,
            "third-variant",
            false,
            "first-variant",
            "first-variant",
            "third-variant",
            false,
            false,
            false,
            "fourth-variant",
            "second-variant",
            "first-variant",
            false,
            false,
            "first-variant",
            "fourth-variant",
            false,
            "first-variant",
            "third-variant",
            "first-variant",
            false,
            false,
            "third-variant",
            false,
            "first-variant",
            false,
            "first-variant",
            "first-variant",
            "third-variant",
            "second-variant",
            "fourth-variant",
            false,
            "first-variant",
            false,
            false,
            false,
            false,
            "second-variant",
            "first-variant",
            "second-variant",
            false,
            "first-variant",
            false,
            "first-variant",
            "first-variant",
            false,
            "first-variant",
            "first-variant",
            "second-variant",
            "third-variant",
            "first-variant",
            "first-variant",
            "first-variant",
            false,
            false,
            false,
            "third-variant",
            false,
            "first-variant",
            "first-variant",
            "first-variant",
            "third-variant",
            "first-variant",
            "first-variant",
            "second-variant",
            "first-variant",
            "fifth-variant",
            "fourth-variant",
            "first-variant",
            "second-variant",
            false,
            "fourth-variant",
            false,
            false,
            false,
            "fourth-variant",
            false,
            false,
            "third-variant",
            false,
            false,
            false,
            "first-variant",
            "third-variant",
            "third-variant",
            "second-variant",
            "first-variant",
            "second-variant",
            "first-variant",
            false,
            "first-variant",
            false,
            false,
            false,
            false,
            false,
            "first-variant",
            "first-variant",
            false,
            "second-variant",
            false,
            false,
            "first-variant",
            false,
            "second-variant",
            "first-variant",
            "first-variant",
            "first-variant",
            "third-variant",
            "second-variant",
            false,
            false,
            "fifth-variant",
            "third-variant",
            false,
            false,
            "first-variant",
            false,
            false,
            false,
            "first-variant",
            "second-variant",
            "third-variant",
            "third-variant",
            false,
            false,
            "first-variant",
            false,
            "third-variant",
            "first-variant",
            false,
            false,
            false,
            false,
            "fourth-variant",
            "first-variant",
            false,
            false,
            false,
            "third-variant",
            false,
            false,
            "second-variant",
            "first-variant",
            false,
            false,
            "second-variant",
            "third-variant",
            "first-variant",
            "first-variant",
            false,
            "first-variant",
            "first-variant",
            false,
            false,
            "second-variant",
            "third-variant",
            "second-variant",
            "third-variant",
            false,
            false,
            "first-variant",
            false,
            false,
            "first-variant",
            false,
            "second-variant",
            false,
            false,
            false,
            false,
            "first-variant",
            false,
            "third-variant",
            false,
            "first-variant",
            false,
            false,
            "second-variant",
            "third-variant",
            "second-variant",
            "fourth-variant",
            "first-variant",
            "first-variant",
            "first-variant",
            false,
            "first-variant",
            false,
            "second-variant",
            false,
            false,
            false,
            false,
            false,
            "first-variant",
            false,
            false,
            false,
            false,
            false,
            "first-variant",
            false,
            "second-variant",
            false,
            false,
            false,
            false,
            "second-variant",
            false,
            "first-variant",
            false,
            "third-variant",
            false,
            false,
            "first-variant",
            "third-variant",
            false,
            "third-variant",
            false,
            false,
            "second-variant",
            false,
            "first-variant",
            "second-variant",
            "first-variant",
            false,
            false,
            false,
            false,
            false,
            "second-variant",
            false,
            false,
            "first-variant",
            "third-variant",
            false,
            "first-variant",
            false,
            false,
            false,
            false,
            false,
            "first-variant",
            "second-variant",
            false,
            false,
            false,
            "first-variant",
            "first-variant",
            "fifth-variant",
            false,
            false,
            false,
            "first-variant",
            false,
            "third-variant",
            false,
            false,
            "second-variant",
            false,
            false,
            false,
            false,
            false,
            "fourth-variant",
            "second-variant",
            "first-variant",
            "second-variant",
            false,
            "second-variant",
            false,
            "second-variant",
            false,
            "first-variant",
            false,
            "first-variant",
            "first-variant",
            false,
            "second-variant",
            false,
            "first-variant",
            false,
            "fifth-variant",
            false,
            "first-variant",
            "first-variant",
            false,
            false,
            false,
            "first-variant",
            false,
            "first-variant",
            "third-variant",
            false,
            false,
            "first-variant",
            "first-variant",
            false,
            false,
            "fifth-variant",
            false,
            false,
            "third-variant",
            false,
            "third-variant",
            "first-variant",
            "first-variant",
            "third-variant",
            "third-variant",
            false,
            "first-variant",
            false,
            false,
            false,
            false,
            false,
            "first-variant",
            false,
            false,
            false,
            false,
            "second-variant",
            "first-variant",
            "second-variant",
            "first-variant",
            false,
            "fifth-variant",
            "first-variant",
            false,
            false,
            "fourth-variant",
            "first-variant",
            "first-variant",
            false,
            false,
            "fourth-variant",
            "first-variant",
            false,
            "second-variant",
            "third-variant",
            "third-variant",
            "first-variant",
            "first-variant",
            false,
            false,
            false,
            "first-variant",
            "first-variant",
            "first-variant",
            false,
            "third-variant",
            "third-variant",
            "third-variant",
            false,
            false,
            "first-variant",
            "first-variant",
            false,
            "second-variant",
            false,
            false,
            "second-variant",
            false,
            "third-variant",
            "first-variant",
            "second-variant",
            "fifth-variant",
            "first-variant",
            "first-variant",
            false,
            "first-variant",
            "fifth-variant",
            false,
            false,
            false,
            "third-variant",
            "first-variant",
            "first-variant",
            "second-variant",
            "fourth-variant",
            "first-variant",
            "second-variant",
            "first-variant",
            false,
            false,
            false,
            "second-variant",
            "third-variant",
            false,
            false,
            "first-variant",
            false,
            false,
            false,
            false,
            false,
            false,
            "first-variant",
            "first-variant",
            false,
            "third-variant",
            false,
            "first-variant",
            false,
            "third-variant",
            "third-variant",
            "first-variant",
            "first-variant",
            false,
            "second-variant",
            false,
            "second-variant",
            "first-variant",
            false,
            false,
            false,
            "second-variant",
            false,
            "third-variant",
            false,
            "first-variant",
            "fifth-variant",
            "first-variant",
            "first-variant",
            false,
            false,
            "first-variant",
            false,
            false,
            false,
            "first-variant",
            "fourth-variant",
            "first-variant",
            "first-variant",
            "first-variant",
            "fifth-variant",
            false,
            false,
            false,
            "second-variant",
            false,
            false,
            false,
            "first-variant",
            "first-variant",
            false,
            false,
            "first-variant",
            "first-variant",
            "second-variant",
            "first-variant",
            "first-variant",
            "first-variant",
            "first-variant",
            "first-variant",
            "third-variant",
            "first-variant",
            false,
            "second-variant",
            false,
            false,
            "third-variant",
            "second-variant",
            "third-variant",
            false,
            "first-variant",
            "third-variant",
            "second-variant",
            "first-variant",
            "third-variant",
            false,
            false,
            "first-variant",
            "first-variant",
            false,
            false,
            false,
            "first-variant",
            "third-variant",
            "second-variant",
            "first-variant",
            "first-variant",
            "first-variant",
            false,
            "third-variant",
            "second-variant",
            "third-variant",
            false,
            false,
            "third-variant",
            "first-variant",
            false,
            "first-variant",
        ];
        foreach (range(0, 999) as $number) {
            $testResult = PostHog::getFeatureFlag('multivariate-flag', sprintf('distinct_id_%s', $number));
            $this->assertEquals($testResult, $result[$number]);
        }
    }

    public function testFeatureFlagsWithFlagDependencies(): void
    {
        // Test flag dependency evaluation without required context throws exception
        $propertyGroup = [
            "type" => "AND",
            "values" => [
                ["type" => "flag", "key" => "parent-flag", "value" => true],
                ["key" => "email", "value" => "test@example.com", "operator" => "exact"]
            ]
        ];
        
        $properties = ["email" => "test@example.com"];
        
        // Should throw InconclusiveMatchException because flag dependencies cannot be evaluated without flags_by_key
        $threwException = false;
        try {
            FeatureFlag::matchPropertyGroup($propertyGroup, $properties, []);
        } catch (InconclusiveMatchException $e) {
            $this->assertStringContainsString("Cannot evaluate flag dependency on 'parent-flag' without flags_by_key and evaluation_cache", $e->getMessage());
            $threwException = true;
        }
        $this->assertTrue($threwException, "Expected InconclusiveMatchException was not thrown");
        
        // Test flag dependency via matchFeatureFlagProperties
        $flag = [
            "key" => "test-flag",
            "filters" => [
                "groups" => [
                    [
                        "properties" => [
                            ["type" => "flag", "key" => "dependency-flag", "value" => true],
                            ["key" => "name", "value" => "test", "operator" => "exact"]
                        ],
                        "rollout_percentage" => 100
                    ]
                ]
            ]
        ];
        
        $properties = ["name" => "test"];
        
        // Should also throw InconclusiveMatchException because flag dependencies need context
        $threwException = false;
        try {
            FeatureFlag::matchFeatureFlagProperties($flag, "test-user", $properties);
        } catch (InconclusiveMatchException $e) {
            $this->assertStringContainsString("Cannot evaluate flag dependency", $e->getMessage());
            $threwException = true;
        }
        $this->assertTrue($threwException, "Expected InconclusiveMatchException was not thrown");
    }

    public function testFallsBackToAPIWhenFlagHasStaticCohort()
    {
        $this->http_client = new MockedHttpClient(
            host: "app.posthog.com",
            flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_WITH_STATIC_COHORT,
            flagsEndpointResponse: MockedResponses::FLAGS_WITH_STATIC_COHORT_RESPONSE
        );

        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );

        $result = $this->client->getFeatureFlag(
            'multi-condition-flag',
            'test-user',
            [],
            ['$geoip_country_code' => 'DE']
        );

        // Should return 'set-1' from API, not 'set-8' from local evaluation
        $this->assertEquals('set-1', $result);

        $this->checkEmptyErrorLogs();
    }

    public function testFallsBackToAPIInGetAllFlagsWhenFlagHasStaticCohort()
    {
        $this->http_client = new MockedHttpClient(
            host: "app.posthog.com",
            flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_WITH_STATIC_COHORT,
            flagsEndpointResponse: MockedResponses::FLAGS_WITH_STATIC_COHORT_RESPONSE
        );

        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );

        $result = $this->client->getAllFlags(
            'test-user',
            [],
            ['$geoip_country_code' => 'DE']
        );

        // Should return flags from API
        $this->assertEquals([
            'multi-condition-flag' => 'set-1'
        ], $result);

        $this->checkEmptyErrorLogs();
    }

    public function testFallsBackToAPIInGetFeatureFlagPayloadWhenFlagHasStaticCohort()
    {
        $this->http_client = new MockedHttpClient(
            host: "app.posthog.com",
            flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_WITH_STATIC_COHORT_FOR_PAYLOAD,
            flagsEndpointResponse: MockedResponses::FLAGS_WITH_STATIC_COHORT_PAYLOAD_RESPONSE
        );

        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );

        $result = $this->client->getFeatureFlagPayload(
            'flag-with-payload',
            'test-user'
        );

        // Should return payload from API, not local evaluation
        $this->assertEquals([
            'message' => 'from-api'
        ], $result);

        $this->checkEmptyErrorLogs();
    }
}
