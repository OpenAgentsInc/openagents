<?php

namespace App\Models;

require_once __DIR__ . "/../Services/Extism/GPBMetadata/Api.php";
// require_once __DIR__ . "/../Services/Extism/Proto/Module.php";

use App\Services\Extism\Proto\Module;
use Extism\Plugin as ExtismPlugin;
use Extism\Manifest;
use Extism\UrlWasmSource;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Plugin extends Model
{
    use HasFactory;

    private function wasmBytes() {
        dd($this->wasm_url);
        return file_get_contents($this->wasm_url);
    }

    public function parse()
    {
        $wasm = new UrlWasmSource("https://cdn.modsurfer.dylibso.com/api/v1/module/0c20c61f67108ebccae1db0be6df7c7d14b2567d5606154278ee390f43e1f408.wasm");
        $manifest = new Manifest($wasm);
        $plugin = new ExtismPlugin($manifest, true);
        $protobufData = $plugin->call("parse_module", file_get_contents("https://github.com/extism/plugins/releases/latest/download/count_vowels.wasm"));
        $sha256 = hash('sha256', $protobufData);
        $module = new Module([
            'hash' => $sha256,
        ]);
        $exports = $module->getExports();
        dd($exports);
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
