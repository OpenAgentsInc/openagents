<?php

namespace App\OpenApi\Responses;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ResponseFactory;

class AutopilotResponse extends ResponseFactory
{
    public function build(): Response
    {
        return Response::ok()
            ->description('Autopilot resource payload')
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::object('data')->properties(
                            Schema::string('id'),
                            Schema::string('handle'),
                            Schema::string('displayName'),
                            Schema::string('status'),
                            Schema::string('visibility'),
                            Schema::integer('ownerUserId'),
                            Schema::string('phase')->nullable(),
                            Schema::string('createdAt')->format(Schema::FORMAT_DATE_TIME)->nullable(),
                            Schema::string('updatedAt')->format(Schema::FORMAT_DATE_TIME)->nullable(),
                        )
                    )
                )
            );
    }
}
