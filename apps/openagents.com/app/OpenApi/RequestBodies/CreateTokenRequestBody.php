<?php

namespace App\OpenApi\RequestBodies;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\RequestBody;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\RequestBodyFactory;

class CreateTokenRequestBody extends RequestBodyFactory
{
    public function build(): RequestBody
    {
        return RequestBody::create()
            ->description('Create a new Sanctum personal access token')
            ->required()
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::string('name')->example('api-cli'),
                        Schema::array('abilities')->items(Schema::string())->example(['chat:read', 'chat:write']),
                        Schema::string('expires_at')->format(Schema::FORMAT_DATE_TIME)->nullable()
                    )->required('name')
                )
            );
    }
}
