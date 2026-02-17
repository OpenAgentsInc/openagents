<?php

namespace App\OpenApi\Parameters;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Parameter;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ParametersFactory;

class ZoneQueryParameter extends ParametersFactory
{
    /**
     * @return array<int, Parameter>
     */
    public function build(): array
    {
        return [
            Parameter::query()
                ->name('zone')
                ->description('Optional shout zone filter (e.g. global, l402, dev).')
                ->required(false)
                ->schema(
                    Schema::string()
                        ->maxLength(64)
                        ->example('global')
                ),
        ];
    }
}
