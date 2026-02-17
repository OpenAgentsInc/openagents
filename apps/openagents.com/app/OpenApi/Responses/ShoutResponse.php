<?php

namespace App\OpenApi\Responses;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ResponseFactory;

class ShoutResponse extends ResponseFactory
{
    public function build(): Response
    {
        return Response::ok()
            ->description('Shout response payload')
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::object('data')->properties(
                            Schema::integer('id'),
                            Schema::string('zone')->nullable(),
                            Schema::string('body'),
                            Schema::string('visibility'),
                            Schema::object('author')->properties(
                                Schema::integer('id'),
                                Schema::string('name'),
                                Schema::string('handle'),
                                Schema::string('avatar')->nullable(),
                            ),
                            Schema::string('createdAt')->format(Schema::FORMAT_DATE_TIME)->nullable(),
                            Schema::string('updatedAt')->format(Schema::FORMAT_DATE_TIME)->nullable(),
                        )
                    )
                )
            );
    }
}
