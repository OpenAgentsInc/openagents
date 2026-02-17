<?php

namespace App\OpenApi\RequestBodies;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\RequestBody;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\RequestBodyFactory;

class ProfileUpdateRequestBody extends RequestBodyFactory
{
    public function build(): RequestBody
    {
        return RequestBody::create()
            ->description('Update profile fields')
            ->required()
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::string('name')->example('Christopher David')
                    )->required('name')
                )
            );
    }
}
