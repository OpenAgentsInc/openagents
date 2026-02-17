<?php

namespace App\OpenApi\Responses;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ResponseFactory;

class UnauthorizedResponse extends ResponseFactory
{
    public function build(): Response
    {
        return Response::unauthorized()
            ->description('Authentication required')
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::string('message')->example('Unauthenticated.')
                    )
                )
            );
    }
}
