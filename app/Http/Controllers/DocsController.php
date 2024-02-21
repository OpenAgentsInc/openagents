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
        'api.md',
    ];

    public function __construct(private Sheets $sheets)
    {
    }

    public function show($page)
    {
        $content = $this->sheets->collection('docs')->get($page);

        if (!$content) {
            abort(404);
        }

        $documentsList = collect($this->docsInOrder)->mapWithKeys(function ($slug) {
            $doc = $this->sheets->collection('docs')->get($slug);
            return [$slug => $doc ? $doc->title : 'Untitled'];
        });

        return view('docs.show', [
            'content' => $content,
            'documentsList' => $documentsList,
            'activePage' => $page,
        ]);
    }
}
