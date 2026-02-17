<?php

namespace App\OpenApi\Responses;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ResponseFactory;

class WhisperResponse extends ResponseFactory
{
    public function build(): Response
    {
        return Response::ok()
            ->description('Whisper response payload')
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::object('data')->properties(
                            Schema::integer('id'),
                            Schema::string('body'),
                            Schema::object('sender')->properties(
                                Schema::integer('id'),
                                Schema::string('name'),
                                Schema::string('handle'),
                                Schema::string('avatar')->nullable(),
                            ),
                            Schema::object('recipient')->properties(
                                Schema::integer('id'),
                                Schema::string('name'),
                                Schema::string('handle'),
                                Schema::string('avatar')->nullable(),
                            ),
                            Schema::string('readAt')->format(Schema::FORMAT_DATE_TIME)->nullable(),
                            Schema::string('createdAt')->format(Schema::FORMAT_DATE_TIME)->nullable(),
                            Schema::string('updatedAt')->format(Schema::FORMAT_DATE_TIME)->nullable(),
                        )
                    )
                )
            );
    }
}
