<?php

namespace App\OpenApi\Responses;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ResponseFactory;

class TokenCreateResponse extends ResponseFactory
{
    public function build(): Response
    {
        return Response::created()
            ->description('Token created; plain token value is returned once')
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::object('data')->properties(
                            Schema::string('token'),
                            Schema::integer('tokenableId'),
                            Schema::string('name'),
                            Schema::array('abilities')->items(Schema::string()),
                            Schema::string('expiresAt')->format(Schema::FORMAT_DATE_TIME)->nullable(),
                        )
                    )
                )
            );
    }
}
