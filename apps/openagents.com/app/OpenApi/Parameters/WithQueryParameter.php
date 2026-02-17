<?php

namespace App\OpenApi\Parameters;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Parameter;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ParametersFactory;

class WithQueryParameter extends ParametersFactory
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
                ->schema(
                    Schema::string()->example('agent:autopilot')
                ),
        ];
    }
}
