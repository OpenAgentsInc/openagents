<?php

namespace App\OpenApi\Parameters;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Parameter;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ParametersFactory;

class SinceQueryParameter extends ParametersFactory
{
    /**
     * @return array<int, Parameter>
     */
    public function build(): array
    {
        return [
            Parameter::query()
                ->name('since')
                ->description('Optional ISO-8601 timestamp for incremental polling.')
                ->required(false)
                ->schema(
                    Schema::string()->format(Schema::FORMAT_DATE_TIME)
                ),
        ];
    }
}
