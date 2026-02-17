<?php

namespace App\OpenApi\Parameters;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Parameter;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ParametersFactory;

class AutopilotQueryParameter extends ParametersFactory
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
        ];
    }
}
