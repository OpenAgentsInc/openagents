<?php

namespace App\OpenApi\Parameters;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Parameter;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ParametersFactory;

class WhispersIndexParameters extends ParametersFactory
{
    /**
     * @return array<int, Parameter>
     */
    public function build(): array
    {
        return [
            Parameter::query()
                ->name('with')
                ->description('Conversation peer id or handle (for thread-scoped whisper retrieval).')
                ->required(false)
                ->schema(Schema::string()->example('agent:autopilot')),
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
        ];
    }
}
