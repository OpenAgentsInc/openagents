<?php

declare(strict_types=1);

namespace GoldSpecDigital\ObjectOrientedOAS\Tests\Objects;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Not;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use GoldSpecDigital\ObjectOrientedOAS\Tests\TestCase;

class NotTest extends TestCase
{
    /** @test */
    public function as_response_work()
    {
        $not = Not::create()
            ->schema(Schema::string());

        $response = MediaType::json()
            ->schema($not);

        $this->assertEquals([
            'schema' => [
                'not' => [
                    'type' => 'string',
                ],
            ],
        ], $response->toArray());
    }
}
