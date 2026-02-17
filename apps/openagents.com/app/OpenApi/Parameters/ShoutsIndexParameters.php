<?php

namespace App\OpenApi\Parameters;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Parameter;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ParametersFactory;

class ShoutsIndexParameters extends ParametersFactory
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
                ->schema(Schema::string()->maxLength(64)->example('global')),
            Parameter::query()
                ->name('limit')
                ->description('Number of records to return (default 50, max effective 200).')
                ->required(false)
                ->schema(Schema::integer()->minimum(1)->example(50)),
            Parameter::query()
                ->name('before_id')
                ->description('Pagination cursor: return rows with id lower than this value.')
                ->required(false)
                ->schema(Schema::integer()->minimum(1)->example(12345)),
            Parameter::query()
                ->name('since')
                ->description('Optional ISO-8601 timestamp for incremental polling.')
                ->required(false)
                ->schema(Schema::string()->format(Schema::FORMAT_DATE_TIME)),
        ];
    }
}
