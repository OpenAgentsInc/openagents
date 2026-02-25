<?php

declare(strict_types=1);

namespace GoldSpecDigital\ObjectOrientedOAS\Tests\Objects;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\OneOf;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use GoldSpecDigital\ObjectOrientedOAS\Tests\TestCase;

class OneOfTest extends TestCase
{
    /** @test */
    public function two_schemas_work()
    {
        $schema1 = Schema::string();
        $schema2 = Schema::integer();

        $oneOf = OneOf::create()
            ->schemas($schema1, $schema2);

        $this->assertEquals([
            'oneOf' => [
                [
                    'type' => 'string',
                ],
                [
                    'type' => 'integer',
                ],
            ],
        ], $oneOf->toArray());
    }

    /** @test */
    public function two_schemas_as_response_work()
    {
        $schema1 = Schema::string();
        $schema2 = Schema::integer();

        $oneOf = OneOf::create()
            ->schemas($schema1, $schema2);

        $response = MediaType::json()
            ->schema($oneOf);

        $this->assertEquals([
            'schema' => [
                'oneOf' => [
                    [
                        'type' => 'string',
                    ],
                    [
                        'type' => 'integer',
                    ],
                ],
            ],
        ], $response->toArray());
    }
}
