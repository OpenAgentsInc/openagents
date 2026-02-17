<?php

namespace App\OpenApi\Responses;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ResponseFactory;

class CreatedDataObjectResponse extends ResponseFactory
{
    public function build(): Response
    {
        return Response::created()
            ->description('Resource created')
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::object('data')
                    )
                )
            );
    }
}
