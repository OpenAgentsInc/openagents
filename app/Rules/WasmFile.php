<?php

namespace App\Rules;

use Closure;
use Illuminate\Contracts\Validation\ValidationRule;

class WasmFile implements ValidationRule
{
    /**
     * Run the validation rule.
     *
     * @param  \Closure(string): \Illuminate\Translation\PotentiallyTranslatedString  $fail
     */
    public function validate(string $attribute, mixed $value, Closure $fail): void
    {
        if($value->getClientOriginalExtension() != 'wasm'){
            $fail('The :attribute must be a wasm file.');
        }

        if($value->getMimeType() != 'application/wasm'){
            $fail('The :attribute must be a valid WebAssembly (.wasm) file.');
        }

    }
}
