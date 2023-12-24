<?php

namespace App\Http;

use Generator;
// use Psr\Http\Message\ResponseInterface;
use Psr\Http\Message\StreamInterface;

class StreamResponse
{
    // private ResponseInterface $response;
    private $body;

    public function __construct($body)
    {
        $this->body = $body;
    }

    public function getIterator(): Generator
    {
        dump("in get iterator");
        while (!$this->body->eof()) {
            $line = $this->readLine($this->response->getBody());
            dump($line);

            if (!str_starts_with($line, 'data:')) {
                continue;
            }

            $data = trim(substr($line, strlen('data:')));

            if ($data === '[DONE]') {
                break;
            }

            $response = json_decode($data, true, JSON_THROW_ON_ERROR);

            if (isset($response['error'])) {
                throw new \Exception($response['error']);
            }

            yield $response;
        }
    }

    private function readLine(StreamInterface $stream): string
    {
        $buffer = '';

        while (!$stream->eof()) {
            if ('' === ($byte = $stream->read(1))) {
                return $buffer;
            }
            $buffer .= $byte;
            if ($byte === "\n") {
                break;
            }
        }

        return $buffer;
    }
}
