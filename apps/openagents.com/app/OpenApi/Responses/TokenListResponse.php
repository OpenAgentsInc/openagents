<?php

namespace App\OpenApi\Responses;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ResponseFactory;

class TokenListResponse extends ResponseFactory
{
    public function build(): Response
    {
        return Response::ok()
            ->description('List personal access tokens for the authenticated user')
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::array('data')->items(
                            Schema::object()->properties(
                                Schema::integer('id'),
                                Schema::string('name'),
                                Schema::array('abilities')->items(Schema::string()),
                                Schema::string('lastUsedAt')->format(Schema::FORMAT_DATE_TIME)->nullable(),
                                Schema::string('expiresAt')->format(Schema::FORMAT_DATE_TIME)->nullable(),
                                Schema::string('createdAt')->format(Schema::FORMAT_DATE_TIME)->nullable(),
                                Schema::boolean('isCurrent')
                            )
                        )
                    )
                )
            );
    }
}
