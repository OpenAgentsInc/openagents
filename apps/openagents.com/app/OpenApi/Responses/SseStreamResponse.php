<?php

namespace App\OpenApi\Responses;

use GoldSpecDigital\ObjectOrientedOAS\Objects\MediaType;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Response;
use GoldSpecDigital\ObjectOrientedOAS\Objects\Schema;
use Vyuldashev\LaravelOpenApi\Factories\ResponseFactory;

class SseStreamResponse extends ResponseFactory
{
    public function build(): Response
    {
        return Response::ok()
            ->description('Server-sent event stream (Vercel AI data stream protocol)')
            ->content(
                MediaType::create()->mediaType('text/event-stream')->schema(
                    Schema::string()->example("data: {\"type\":\"start\"}\n\ndata: [DONE]\n\n")
                )
            );
    }
}
