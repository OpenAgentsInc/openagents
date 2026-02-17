<?php

namespace App\OpenApi\Parameters;

use GoldSpecDigital\ObjectOrientedOAS\Objects\Parameter;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ParametersFactory;

class ChatLimitQueryParameter extends ParametersFactory
{
    /**
     * @return Parameter[]
     */
    public function build(): array
    {
        return [
            Parameter::query()
                ->name('chat_limit')
                ->description('Maximum number of recent chat threads to include.')
                ->required(false)
                ->schema(Schema::integer()->minimum(1)->maximum(200)->example(50)),
        ];
    }
}
