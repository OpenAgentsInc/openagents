<?php

namespace App\OpenApi\Responses;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ResponseFactory;

class MeResponse extends ResponseFactory
{
    public function build(): Response
    {
        return Response::ok()
            ->description('Authenticated user profile and quick chat thread summary')
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::object('data')->properties(
                            Schema::object('user')->properties(
                                Schema::integer('id'),
                                Schema::string('name'),
                                Schema::string('email'),
                                Schema::string('avatar')->nullable(),
                                Schema::string('createdAt')->format(Schema::FORMAT_DATE_TIME)->nullable(),
                                Schema::string('updatedAt')->format(Schema::FORMAT_DATE_TIME)->nullable(),
                            ),
                            Schema::boolean('isAdmin'),
                            Schema::array('chatThreads')->items(
                                Schema::object()->properties(
                                    Schema::string('id'),
                                    Schema::string('title'),
                                    Schema::string('updatedAt')->format(Schema::FORMAT_DATE_TIME)->nullable(),
                                )
                            )
                        )
                    )
                )
            );
    }
}
