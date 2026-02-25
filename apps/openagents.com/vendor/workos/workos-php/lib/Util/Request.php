<?php

namespace WorkOS\Util;

class Request
{
    public static function parsePaginationArgs($response)
    {
        return [
            $response["list_metadata"]["before"],
            $response["list_metadata"]["after"]
        ];
    }
}
