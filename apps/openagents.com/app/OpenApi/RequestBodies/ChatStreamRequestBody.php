<?php

namespace App\OpenApi\RequestBodies;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\RequestBody;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\RequestBodyFactory;

class ChatStreamRequestBody extends RequestBodyFactory
{
    public function build(): RequestBody
    {
        return RequestBody::create()
            ->description('Full client-side message history; backend executes using the latest user message. L402 tool contract: `maxSpendMsats` is canonical (`maxSpendSats` temporary alias) and `requireApproval` is canonical (`approvalRequired` temporary alias).')
            ->required()
            ->content(
                MediaType::json()->schema(
                    Schema::object()->properties(
                        Schema::array('messages')->items(
                            Schema::object()->properties(
                                Schema::string('id')->nullable(),
                                Schema::string('role')->example('user'),
                                Schema::string('content')->nullable(),
                                Schema::array('parts')->items(
                                    Schema::object()->properties(
                                        Schema::string('text')->nullable(),
                                        Schema::string('type')->nullable()
                                    )
                                )->nullable()
                            )
                        )
                    )->required('messages')
                )
            );
    }
}
