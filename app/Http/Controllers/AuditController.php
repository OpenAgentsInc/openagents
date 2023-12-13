<?php

namespace App\Http\Controllers;

use App\Models\Agent;
use App\Services\Auditor;
use Inertia\Inertia;
use Inertia\Response;

class AuditController extends Controller
{
    public function store()
    {
        request()->validate([
            'repo' => 'required',
        ]);

        $repo = request('repo');

        // Split repo into owner and name
        [$owner, $name] = explode('/', $repo);

        // Initialize Auditor with those two vars
        $auditor = new Auditor($owner, $name);
        // $repo = $auditor->getRepo();
        $contents = $auditor->getFolderContents();
        dd($contents);

        return redirect()->back()->with('message', 'Auditor initialized');
    }
}
