<?php

namespace App\Http\Controllers;

use App\Models\Plugin;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Validator;
use Mauricius\LaravelHtmx\Http\HtmxResponse;

class PluginController extends Controller
{
    public function index()
    {
        return view('plugins', [
            'plugins' => Plugin::all(),
        ]);
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

        return "Plugin uploaded successfully.";
    }
}
