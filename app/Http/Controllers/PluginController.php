<?php

namespace App\Http\Controllers;

use App\Models\Plugin;
use Illuminate\Http\Response;
use Illuminate\Support\Facades\Validator;
use Mauricius\LaravelHtmx\Http\HtmxResponse;

class PluginController extends Controller
{
    public function call()
    {
        $plugin = Plugin::find(request('plugin_id'));

        switch ($plugin->name) {
            case 'URL Extractor':
                $function = 'extract_urls';
                $input = request('input');
                break;
            case 'URL Scraper':
                $function = 'fetch_url_content';
                $input = json_encode([
                    'url' => request('input'),
                ]);
                break;
            case 'LLM Inferencer':
                $function = 'inference';
                $input = json_encode([
                    'model_name' => 'gpt-4',
                    'input_content' => request('input'),
                    'api_key' => env('OPENAI_API_KEY'),
                ]);
                break;
            default:
                dd("Unknown plugin: {$plugin->name}");
        }

        if (! $plugin) {
            return new Response('Plugin not found', 404);
        }

        $output = $plugin->call($function, $input);

        return response()->json([
            'output' => $output,
        ]);

        // return with(new HtmxResponse())
        //     ->renderFragment('plugin-call-output', 'output', compact('output'));
    }

    public function index()
    {
        return view('plugins', [
            'plugins' => Plugin::all(),
        ]);
    }

    public function show($pluginId)
    {
        $plugin = Plugin::find($pluginId);

        if (! $plugin) {
            return redirect('/');
        }

        return view('plugin-show', [
            'plugin' => $plugin,
        ]);
    }

    public function create()
    {
        return view('plugin-create');
    }

    public function store()
    {
        $validator = Validator::make(request()->all(), [
            'name' => 'required',
            'fee' => 'required|numeric',
            'description' => 'required',
            'wasm_url' => 'required|url|active_url',
        ]);

        if ($validator->fails()) {
            return view('plugin-upload-failed', [
                'errors' => $validator->errors(),
            ]);
        }

        $plugin = Plugin::create([
            'name' => request('name'),
            'fee' => request('fee'),
            'description' => request('description'),
            'wasm_url' => request('wasm_url'),
        ]);

        return redirect()->route('plugins.show', $plugin);
    }
}
