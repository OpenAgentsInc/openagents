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

    private function wasmBytes() {
        return file_get_contents($this->wasm_url);
    }

    public function parse()
    {
        $wasm = new UrlWasmSource("https://cdn.modsurfer.dylibso.com/api/v1/module/0c20c61f67108ebccae1db0be6df7c7d14b2567d5606154278ee390f43e1f408.wasm");
        $manifest = new Manifest($wasm);

        $plugin = new ExtismPlugin($manifest, true);
        $protobufData = $plugin->call("parse_module", $this->wasmBytes());
        dd($protobufData);
    }

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
