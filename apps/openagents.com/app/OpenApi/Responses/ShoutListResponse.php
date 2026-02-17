<?php

namespace App\OpenApi\Responses;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ResponseFactory;

class ShoutListResponse extends ResponseFactory
{
    public function build(): Response
    {
        return Response::ok()
            ->description('Shout feed response')
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::array('data')->items(
                            Schema::object()->properties(
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
                        ),
                        Schema::object('meta')->properties(
                            Schema::string('nextCursor')->nullable(),
                        ),
                    )
                )
            );
    }
}
