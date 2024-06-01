<?php

namespace App\Rules;

use Closure;
use Illuminate\Contracts\Validation\ValidationRule;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Str;

class WasmUrl implements ValidationRule
{
    /**
     * Run the validation rule.
     *
     * @param  \Closure(string): \Illuminate\Translation\PotentiallyTranslatedString  $fail
     */
    public function validate(string $attribute, mixed $value, Closure $fail): void
    {
        // Download the file
        $response = Http::get($value);

        // Check if the request was successful
        if ($response->failed()) {
            $fail(__('The :attribute must be accesible file', ['attribute' => $attribute]));
        }

        $ch = curl_init($value);
        curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);
        $response = curl_exec($ch);
        $httpCode = curl_getinfo($ch, CURLINFO_HTTP_CODE);
        curl_close($ch);

        if (! Str::endsWith($value, '.wasm')) {
            $fail(__('The :attribute must be a valid WASM file', ['attribute' => $attribute]));
        }

        // Check MIME type
        // $mimeType = $response->header('Content-Type');
        // if ($mimeType != 'application/wasm') {
        //     $fail(__('The :attribute must be a valid (application/wasm) file', ['attribute' => $attribute]));
        // }

        // Check the file signature
        // $fileContent = $response->body();
        // $fileContent = file_get_contents($value);
        // if( substr($fileContent, 0, 4) != "\0asm"){
        //     $fail(__('The :attribute must be a valid WASM file', ['attribute' => $attribute]));
        // }
    }
}
