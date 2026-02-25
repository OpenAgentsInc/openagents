<?php

declare(strict_types=1);

namespace GoldSpecDigital\ObjectOrientedOAS\Tests\Objects;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Operation;
use GoldSpecDigital\ObjectOrientedOAS\Objects\RequestBody;
use GoldSpecDigital\ObjectOrientedOAS\Tests\TestCase;

class RequestBodyTest extends TestCase
{
    /** @test */
    public function create_with_all_parameters_works()
    {
        $requestBody = RequestBody::create()
            ->description('Standard request')
            ->content(MediaType::json())
            ->required();

        $operation = Operation::create()
            ->requestBody($requestBody);

        $this->assertEquals([
            'requestBody' => [
                'description' => 'Standard request',
                'content' => [
                    'application/json' => [],
                ],
                'required' => true,
            ],
        ], $operation->toArray());
    }
}
