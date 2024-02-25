<?php

namespace App\Http\Controllers;

use Illuminate\Support\Facades\File;
use Illuminate\Support\Str;
use Spatie\Sheets\Sheets;

class DocsController extends Controller
{
    protected $docsInOrder = [
        'introduction.md',
        'plugins.md',
        'agentgraph.md',
        'payments.md',
        'screencasts.md',
        'bounties.md'
    ];

    protected $apiDocsInOrder = [
        'agents.md',
        'conversations.md',
        'messages.md',
        'files.md',
    ];

    public function __construct(private Sheets $sheets)
    {
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
