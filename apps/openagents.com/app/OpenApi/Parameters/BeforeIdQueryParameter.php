<?php

namespace App\OpenApi\Parameters;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Parameter;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ParametersFactory;

class BeforeIdQueryParameter extends ParametersFactory
{
    /**
     * @return array<int, Parameter>
     */
    public function build(): array
    {
        return [
            Parameter::query()
                ->name('before_id')
                ->description('Pagination cursor: return rows with id lower than this value.')
                ->required(false)
                ->schema(
                    Schema::integer()->minimum(1)->example(12345)
                ),
        ];
    }
}
