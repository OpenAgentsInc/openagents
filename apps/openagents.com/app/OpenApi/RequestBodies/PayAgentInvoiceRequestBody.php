<?php

namespace App\OpenApi\RequestBodies;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\RequestBody;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\RequestBodyFactory;

class PayAgentInvoiceRequestBody extends RequestBodyFactory
{
    public function build(): RequestBody
    {
        return RequestBody::create()
            ->description('Pay a BOLT11 invoice from the authenticated user wallet.')
            ->required()
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::string('invoice')->example('lnbc10u1...'),
                        Schema::integer('maxAmountSats')->nullable()->example(100),
                        Schema::integer('maxAmountMsats')->nullable()->example(100000),
                        Schema::integer('timeoutMs')->nullable()->example(12000),
                        Schema::string('host')->nullable()->example('sats4ai.com')
                    )->required('invoice')
                )
            );
    }
}
