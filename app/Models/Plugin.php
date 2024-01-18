<?php

namespace App\Models;

require_once __DIR__ . "/../Services/Extism/GPBMetadata/Api.php";

use App\Services\Extism\Proto\Module;
use App\Services\Extism\Proto\SourceLanguage;
use Extism\Plugin as ExtismPlugin;
use Extism\Manifest;
use Extism\UrlWasmSource;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Plugin extends Model
{
    use HasFactory;

    protected $guarded = [];

    private function wasmBytes()
    {
        return file_get_contents($this->wasm_url);
    }

    public function parse()
    {
        $wasm = new UrlWasmSource("https://cdn.modsurfer.dylibso.com/api/v1/module/0c20c61f67108ebccae1db0be6df7c7d14b2567d5606154278ee390f43e1f408.wasm");
        $manifest = new Manifest($wasm);
        $plugin = new ExtismPlugin($manifest, true);
        $protobufData = $plugin->call("parse_module", $this->wasmBytes());
        $module = new Module();
        $module->mergeFromString($protobufData);

        $exports = $module->getExports();
        $moduleMetadata = [
            'module_id' => $module->getId(),
            'module_hash' => $module->getHash(),
            'exports_count' => count($exports),
            'size' => $module->getSize(),
            'module_location' => $module->getLocation(),
            'source_language' => SourceLanguage::name($module->getSourceLanguage()),
            'exports' => []
        ];

        foreach ($exports as $export) {
            if ($export->hasFunc()) {
                $function = $export->getFunc(); // $function is an instance of PBFunction
                $moduleMetadata['exports'][] = $function->getName();
            }
        }

        return $moduleMetadata;
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
        $parsed = $this->parse();
        return $parsed['exports'];
    }
}
