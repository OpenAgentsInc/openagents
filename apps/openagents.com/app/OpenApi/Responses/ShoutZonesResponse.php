<?php

namespace App\OpenApi\Responses;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ResponseFactory;

class ShoutZonesResponse extends ResponseFactory
{
    public function build(): Response
    {
        return Response::ok()
            ->description('Top shout zones by last-24h activity')
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::array('data')->items(
                            Schema::object()->properties(
                                Schema::string('zone'),
                                Schema::integer('count24h'),
                            )
                        )
                    )
                )
            );
    }
}
