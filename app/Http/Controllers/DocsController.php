<?php

namespace App\Http\Controllers;

use Spatie\Sheets\Sheets;

class DocsController extends Controller
{
    public function show($page, Sheets $sheets)
    {
        $content = $sheets->collection('docs')->get($page);

        if (! $content) {
            abort(404);
        }

        return view('docs.show', ['content' => $content]);
    }
}
