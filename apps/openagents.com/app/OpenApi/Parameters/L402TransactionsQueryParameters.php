<?php

namespace App\OpenApi\Parameters;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Parameter;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ParametersFactory;

class L402TransactionsQueryParameters extends ParametersFactory
{
    /**
     * @return Parameter[]
     */
    public function build(): array
    {
        return [
            Parameter::query()
                ->name('autopilot')
                ->description('Optional autopilot scope filter by owned autopilot id or handle.')
                ->required(false)
                ->schema(Schema::string()->minLength(1)->maxLength(128)->example('ep212-bot')),
            Parameter::query()
                ->name('per_page')
                ->description('Number of transaction rows per page (1-200).')
                ->required(false)
                ->schema(Schema::integer()->minimum(1)->maximum(200)->example(30)),
        ];
    }
}
