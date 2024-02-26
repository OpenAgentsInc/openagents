<?php

namespace App\Http\Controllers;

use Illuminate\Support\Facades\File;
use Illuminate\Support\Str;
use Spatie\Sheets\Sheets;
use Spatie\ShikiPhp\Shiki;

class DocsController extends Controller
{
    protected $docsInOrder = [
        'introduction.md',
        'agentgraph.md',
        'payments.md',
        'bounties.md',
        'screencasts.md',
    ];

    protected $apiDocsInOrder = [
        'api-overview.md',
        'agents.md',
        'threads.md',
        'messages.md',
        'files.md',
        'runs.md',
        'flows.md',
        'nodes.md',
        'plugins.md',
    ];

    public function __construct(private Sheets $sheets)
    {
    }

<<<<<<< HEAD
    public function show($piage)
=======
    public function apidoc()
    {
        $content = $this->sheets->collection('docs')->get('agents');

        $curl = Shiki::highlight(
            code: $content->curl,
            language: 'shell',
            theme: 'tokyo-night',
        );

        $response = Shiki::highlight(
            code: $content->responses[0]["200"],
            language: 'json',
            theme: 'tokyo-night',
        );

        $response2 = Shiki::highlight(
            code: $content->responses[1]["400"],
            language: 'json',
            theme: 'tokyo-night',
        );

        return view('apidoc', [
            'content' => $content,
            'curl' => $curl,
            'responseSuccessHtml' => $response,
            'responseErrorHtml' => $response2,
        ]);
    }

    public function show($page)
>>>>>>> 0648a2afb25c57cd1bfdd08f6e01376a313be58b
    {
        // Trim .md from page slug for fetching content
        $contentSlug = Str::before($page, '.md');
        $content = $this->sheets->collection('docs')->get($contentSlug);

        dd($content);

        if (!$content) {
            abort(404);
        }

        $documentsList = collect($this->docsInOrder)->mapWithKeys(function ($filePath) {
            $slug = Str::before($filePath, '.md'); // Remove .md extension
            $doc = $this->sheets->collection('docs')->get($slug);
            return [$slug => $doc ? $doc->title : 'Untitled'];
        });

        $apiDocumentsList = collect($this->apiDocsInOrder)->mapWithKeys(function ($filePath) {
            $slug = Str::before($filePath, '.md'); // Remove .md extension
            $doc = $this->sheets->collection('docs')->get($slug);
            return [$slug => $doc ? $doc->title : 'Untitled'];
        });

        $activePage = $contentSlug;

        return view('docs.show', [
            'content' => $content,
            'documentsList' => $documentsList,
            'apiDocumentsList' => $apiDocumentsList,
            'activePage' => $activePage,
        ]);
    }
}
