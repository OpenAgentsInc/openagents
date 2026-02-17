<?php

namespace App\OpenApi\Parameters;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Parameter;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ParametersFactory;

class LimitQueryParameter extends ParametersFactory
{
    /**
     * @return Parameter[]
     */
    public function build(): array
    {
        return [
            Parameter::query()
                ->name('limit')
                ->description('Maximum records to return (endpoint specific caps apply).')
                ->required(false)
                ->schema(Schema::integer()->minimum(1)->example(100)),
        ];
    }
}
