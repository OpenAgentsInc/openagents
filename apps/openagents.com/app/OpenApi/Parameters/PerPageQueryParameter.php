<?php

namespace App\OpenApi\Parameters;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Parameter;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ParametersFactory;

class PerPageQueryParameter extends ParametersFactory
{
    /**
     * @return Parameter[]
     */
    public function build(): array
    {
        return [
            Parameter::query()
                ->name('per_page')
                ->description('Paginated result size (1-200).')
                ->required(false)
                ->schema(Schema::integer()->minimum(1)->maximum(200)->example(30)),
        ];
    }
}
