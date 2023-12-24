<?php

namespace App\Http;

use Generator;
use Psr\Http\Message\ResponseInterface;
use Psr\Http\Message\StreamInterface;

class StreamResponse
{
    private ResponseInterface $response;

    public function __construct(ResponseInterface $response)
    {
        $this->response = $response;
    }

    public function getIterator(): Generator
    {
        while (!$this->response->getBody()->eof()) {
            $line = $this->readLine($this->response->getBody());

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
