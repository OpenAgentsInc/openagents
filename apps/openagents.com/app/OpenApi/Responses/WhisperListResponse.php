<?php

namespace App\OpenApi\Responses;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ResponseFactory;

class WhisperListResponse extends ResponseFactory
{
    public function build(): Response
    {
        return Response::ok()
            ->description('Whisper inbox/thread response')
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::array('data')->items(
                            Schema::object()->properties(
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
                        ),
                        Schema::object('meta')->properties(
                            Schema::string('nextCursor')->nullable(),
                            Schema::string('with')->nullable(),
                        ),
                    )
                )
            );
    }
}
