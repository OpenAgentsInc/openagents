<?php

namespace App\Models;

require_once __DIR__ . "/../Services/Extism/GPBMetadata/Api.php";

use App\Services\Extism\Proto\Module;
use Extism\Plugin as ExtismPlugin;
use Extism\Manifest;
use Extism\UrlWasmSource;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Plugin extends Model
{
    use HasFactory;

    private function wasmBytes()
    {
        return file_get_contents($this->wasm_url);
    }

    public function parse()
    {
        $valTypeMap = [
            0 => 'I32',
            1 => 'I64',
            2 => 'F32',
            3 => 'F64',
            4 => 'V128',
            5 => 'FuncRef',
            6 => 'ExternRef'
        ];

        $wasm = new UrlWasmSource("https://cdn.modsurfer.dylibso.com/api/v1/module/0c20c61f67108ebccae1db0be6df7c7d14b2567d5606154278ee390f43e1f408.wasm");
        $manifest = new Manifest($wasm);
        $plugin = new ExtismPlugin($manifest, true);
        $protobufData = $plugin->call("parse_module", $this->wasmBytes());
        $module = new Module();
        $module->mergeFromString($protobufData);

        $moduleId = $module->getId();
        $moduleHash = $module->getHash();
        $exports = $module->getExports();
        $exportsCount = count($exports);
        $moduleSize = $module->getSize();
        $moduleLocation = $module->getLocation();
        $sourceLanguage = $module->getSourceLanguage(); // This will be an enum value
        $metadata = $module->getMetadata();

        // Display or process the metadata
        echo "Module ID: $moduleId\n";
        echo "Module Hash: $moduleHash\n";
        echo "Exports Count: $exportsCount\n";
        echo "Module Size: $moduleSize bytes\n";
        echo "Module Location: $moduleLocation\n";

        foreach ($exports as $export) {
            if ($export->hasFunc()) {
                $function = $export->getFunc(); // $function is an instance of PBFunction
                $functionName = $function->getName();
                echo "Function Name: $functionName\n";
            }
        }
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
