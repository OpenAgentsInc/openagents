<?php

namespace App\OpenApi\RequestBodies;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\RequestBody;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\RequestBodyFactory;

class CreateAutopilotRequestBody extends RequestBodyFactory
{
    public function build(): RequestBody
    {
        return RequestBody::create()
            ->description('Create an autopilot resource.')
            ->required()
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::string('handle')->nullable()->maxLength(64)->example('ep212-bot'),
                        Schema::string('displayName')->nullable()->maxLength(120)->example('EP212 Bot'),
                        Schema::string('status')->nullable()->example('active'),
                        Schema::string('visibility')->nullable()->example('private'),
                        Schema::string('avatar')->nullable()->maxLength(255),
                        Schema::string('tagline')->nullable()->maxLength(255),
                    )
                )
            );
    }
}
