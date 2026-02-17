<?php

namespace App\OpenApi\Responses;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ResponseFactory;

class ValidationErrorResponse extends ResponseFactory
{
    public function build(): Response
    {
        return Response::unprocessableEntity()
            ->description('Validation error')
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::string('message')->example('The given data was invalid.'),
                        Schema::object('errors')->additionalProperties(
                            Schema::array()->items(Schema::string())
                        )
                    )
                )
            );
    }
}
