<?php

namespace App\OpenApi\Responses;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ResponseFactory;

class AutopilotThreadListResponse extends ResponseFactory
{
    public function build(): Response
    {
        return Response::ok()
            ->description('Autopilot thread list payload')
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::array('data')->items(
                            Schema::object()->properties(
                                Schema::string('id'),
                                Schema::string('autopilotId'),
                                Schema::string('title'),
                                Schema::string('createdAt')->format(Schema::FORMAT_DATE_TIME)->nullable(),
                                Schema::string('updatedAt')->format(Schema::FORMAT_DATE_TIME)->nullable(),
                            )
                        )
                    )
                )
            );
    }
}
