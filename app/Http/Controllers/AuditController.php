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

        try {
            $repo = request('repo');
            [$owner, $name] = explode('/', $repo);
            $auditor = new Auditor($owner, $name);
            $auditor->audit();
            return redirect()->back()->with('message', 'Auditing');
        } catch (\Exception $e) {
            return redirect()->back()->with('message', 'Error: '. $e->getMessage());
        }
    }
}
