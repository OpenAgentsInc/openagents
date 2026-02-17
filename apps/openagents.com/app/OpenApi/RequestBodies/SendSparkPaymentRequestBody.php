<?php

namespace App\OpenApi\RequestBodies;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\RequestBody;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\RequestBodyFactory;

class SendSparkPaymentRequestBody extends RequestBodyFactory
{
    public function build(): RequestBody
    {
        return RequestBody::create()
            ->description('Send sats to a Spark address from the authenticated user wallet.')
            ->required()
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::string('sparkAddress')->example('someone@spark.wallet'),
                        Schema::integer('amountSats')->example(21),
                        Schema::integer('timeoutMs')->nullable()->example(12000)
                    )->required('sparkAddress', 'amountSats')
                )
            );
    }
}
