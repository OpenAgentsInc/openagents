<?php

namespace App\Http\Controllers;

use Illuminate\Support\Facades\File;
use Inertia\Inertia;
use Spatie\LaravelMarkdown\MarkdownRenderer;

class ContentController extends Controller
{
    public function thesis(MarkdownRenderer $markdown)
    {
        $path = resource_path('markdown/thesis.md');
        
        if (!File::exists($path)) {
            abort(404);
        }

        $content = File::get($path);
        $html = $markdown->toHtml($content);

        return Inertia::render('Content/Show', [
            'content' => $html,
            'title' => 'Thesis'
        ]);
    }

    public function legal(MarkdownRenderer $markdown)
    {
        $path = resource_path('markdown/terms.md');
        
        if (!File::exists($path)) {
            abort(404);
        }

        $content = File::get($path);
        $html = $markdown->toHtml($content);

        return Inertia::render('Legal', [
            'content' => $html
        ]);
    }
}