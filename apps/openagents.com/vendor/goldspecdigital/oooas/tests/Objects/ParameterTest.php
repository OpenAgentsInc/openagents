<?php

declare(strict_types=1);

namespace GoldSpecDigital\ObjectOrientedOAS\Tests\Objects;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Example;
use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Operation;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Parameter;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use GoldSpecDigital\ObjectOrientedOAS\Tests\TestCase;

class ParameterTest extends TestCase
{
    /** @test */
    public function create_with_all_parameters_works()
    {
        $parameter = Parameter::create()
            ->name('user')
            ->in(Parameter::IN_PATH)
            ->description('User ID')
            ->required()
            ->deprecated()
            ->allowEmptyValue()
            ->style(Parameter::STYLE_SIMPLE)
            ->explode()
            ->allowReserved()
            ->schema(Schema::string())
            ->example(Example::create())
            ->examples(Example::create('ExampleName'))
            ->content(MediaType::json());

        $operation = Operation::create()
            ->parameters($parameter);

        $this->assertEquals([
            'parameters' => [
                [
                    'name' => 'user',
                    'in' => 'path',
                    'description' => 'User ID',
                    'required' => true,
                    'deprecated' => true,
                    'allowEmptyValue' => true,
                    'style' => 'simple',
                    'explode' => true,
                    'allowReserved' => true,
                    'schema' => ['type' => 'string'],
                    'example' => [],
                    'examples' => [
                        'ExampleName' => [],
                    ],
                    'content' => [
                        'application/json' => [],
                    ],
                ],
            ],
        ], $operation->toArray());
    }
}
