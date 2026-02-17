<?php

namespace App\OpenApi\RequestBodies;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\RequestBody;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\RequestBodyFactory;

class CreateWhisperRequestBody extends RequestBodyFactory
{
    public function build(): RequestBody
    {
        return RequestBody::create()
            ->description('Send a direct whisper to a user by id or handle.')
            ->required()
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::integer('recipientId')->nullable()->example(42),
                        Schema::string('recipientHandle')->nullable()->example('agent:autopilot'),
                        Schema::string('body')->maxLength(5000)->example('hey'),
                    )
                )
            );
    }
}
