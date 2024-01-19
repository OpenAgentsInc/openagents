<?php

namespace Database\Seeders;

use App\Models\Plugin;
use Illuminate\Database\Seeder;

class PluginSeeder extends Seeder
{
    /**
     * Run the database seeds.
     */
    public function run(): void
    {
        // Seed demo plugins
        Plugin::create([
            'name' => 'Check Bitcoin Price',
            'description' => "Fetch the current Bitcoin price",
            'fee' => 2, // example fee in sats
            'wasm_url' => 'https://example.com/plugin1.wasm',
        ]);

        Plugin::create([
            'name' => 'Create Embedding',
            'description' => "Create vector embedding for a given text",
            'fee' => 5, // example fee in sats
            'wasm_url' => 'https://example.com/plugin2.wasm',
        ]);

        Plugin::create([
            'name' => 'Get GitHub Repo Folder Hierarchy',
            'description' => "Get the folder hierarchy of a GitHub repo",
            'fee' => 400, // example fee in sats
            'wasm_url' => 'https://example.com/plugin3.wasm',
        ]);

        Plugin::create([
            'name' => 'Comment on GitHub Issue',
            'description' => "Generate a comment for a GitHub issue via LLM",
            'fee' => 700, // example fee in sats
            'wasm_url' => 'https://example.com/plugin4.wasm',
        ]);
    }
}
