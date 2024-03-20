<?php

namespace App\Models;

require_once __DIR__.'/../Services/Extism/GPBMetadata/Api.php';

use Extism\Manifest;
use Extism\Plugin as ExtismPlugin;
use Extism\UrlWasmSource;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Plugin extends Model
{
    use HasFactory;

    protected $fillable = ['name', 'description', 'wasm_url'];

    private $plugin = null; // Keep the plugin instance

    public function call(string $function, string $input): mixed
    {
        if (! $this->plugin) {
            $this->initializePlugin();
        }

        return $this->plugin->call($function, $input);
    }

    // optionally pass in functions to go to the plugin
    public function initializePlugin($functions = [])
    {
        if (! $this->wasm_url) {
            dd('wasm_url is not set');

            return; // Ensure wasm_url is set
        }

        $wasm = new UrlWasmSource($this->wasm_url);
        $manifest = new Manifest($wasm);
        $manifest->allowed_hosts = ['*'];
        $this->plugin = new ExtismPlugin($manifest, true, $functions);
    }
}
