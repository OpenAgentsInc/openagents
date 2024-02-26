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

    public function apidoc($page)
    {
        $content = $this->sheets->collection('docs')->get('api-' . $page);

        $curl = Shiki::highlight(
            code: $content->curl,
            language: 'shell',
            theme: 'tokyo-night',
        );

        $responseContent = $content->responses["200"]['content']['application/json']['schema'];
        $response = Shiki::highlight(
            code: json_encode($responseContent, JSON_PRETTY_PRINT),
            language: 'json',
            theme: 'tokyo-night',
        );

        $responseContent2 = $content->responses["400"]['content']['application/json']['schema'];
        $response2 = Shiki::highlight(
            code: json_encode($responseContent2, JSON_PRETTY_PRINT),
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
    {
        // Trim .md from page slug for fetching content
        $contentSlug = Str::before($page, '.md');
        $content = $this->sheets->collection('docs')->get($contentSlug);

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
