<?php

namespace App\Http\Controllers;

use App\Models\Plugin;
use Illuminate\Http\Request;
use Mauricius\LaravelHtmx\Http\HtmxResponse;

class PluginController extends Controller
{
    public function index()
    {
        return view('plugins');
    }

    public function store()
    {
        Plugin::create([
            'name' => request('name'),
            'description' => request('description'),
            'wasm_url' => request('wasm_url'),
        ]);

        return "Plugin uploaded successfully.";
    }
}
