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
            $run_id = $auditor->run->id;
            $auditor->dispatchAuditJob();
            return redirect()->route('inspect-run', $run_id);
        } catch (\Exception $e) {
            return redirect()->back()->with('message', 'Error: '. $e->getMessage());
        }
    }
}
