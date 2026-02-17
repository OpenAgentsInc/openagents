<?php

namespace App\OpenApi\RequestBodies;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\RequestBody;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\RequestBodyFactory;

class UpsertAgentWalletRequestBody extends RequestBodyFactory
{
    public function build(): RequestBody
    {
        return RequestBody::create()
            ->description('Create a new Spark wallet for the current user, or import an existing one with a mnemonic.')
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::string('mnemonic')->nullable()->example('abandon ability able about above absent absorb abstract absurd abuse access accident')
                    )
                )
            );
    }
}
