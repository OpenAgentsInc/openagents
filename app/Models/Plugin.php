<?php

namespace App\Models;

use Extism\Plugin as ExtismPlugin;
use Extism\Manifest;
use Extism\UrlWasmSource;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Plugin extends Model
{
    use HasFactory;

    public function call(string $function, string $input): mixed
    {
        $wasm = new UrlWasmSource($this->wasm_url);
        $manifest = new Manifest($wasm);

        $plugin = new ExtismPlugin($manifest, true);
        return $plugin->call($function, $input);
    }

    public function functions(): array
    {
        $wasm = new UrlWasmSource($this->wasm_url);
        $manifest = new Manifest($wasm);

        $plugin = new ExtismPlugin($manifest, true);
        // dd($plugin->functionExists("count_vowels"));
        return $plugin->functions();
    }
}
