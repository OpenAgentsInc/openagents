<?php

namespace App\Http\Controllers;

use Illuminate\Http\Response;
use Vyuldashev\LaravelOpenApi\Generator;

class OpenApiSpecController extends Controller
{
    public function show(Generator $generator): Response
    {
        $json = null;

        if (! app()->environment('testing')) {
            $path = public_path('openapi.json');

            if (is_file($path) && is_readable($path)) {
                $contents = file_get_contents($path);
                if (is_string($contents) && $contents !== '') {
                    $json = $contents;
                }
            }
        }

        if (! is_string($json) || $json === '') {
            $json = $generator->generate()->toJson(JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
        } else {
            $decoded = json_decode($json, true);
            if (json_last_error() === JSON_ERROR_NONE) {
                $json = json_encode($decoded, JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
            }
        }

        return response($json ?? '{}', 200, [
            'Content-Type' => 'application/json; charset=UTF-8',
            'Cache-Control' => 'public, max-age=60',
        ]);
    }
}
