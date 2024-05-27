<?php

namespace App\Livewire\Plugins;

use App\Models\User;
use App\Models\Plugin;
use App\Rules\WasmUrl;
use Livewire\Component;
use Illuminate\Support\Collection;
use Illuminate\Support\Facades\Log;
use Jantinnerezo\LivewireAlert\LivewireAlert;

class PluginCreate extends Component
{

    use LivewireAlert;

    public $kind;
    public $name;
    public $description;
    public $tos;
    public $privacy;
    public $author;
    public $web;
    public $picture;
    public $tags;
    public $mini_template;
    public $sockets;
    public $input;
    public Collection $inputs;
    public $outputs = [];
    public $file_link = 'http://';
    public $output_description;
    public $output_type;

    public function mount()
    {


        $this->fill([
            'inputs' => collect([[
                'name' => '',
                'required' => true,
                'type' => 'string',
                'description' => ''
            ]]),
        ]);
    }

    public function rules()
    {
        return [
            // 'kind' => 'required|string',
            'name' => 'required|string',
            'description' => 'required|string',
            'tos' => 'required|string',
            'privacy' => 'required|string',
            'author' => 'nullable|string',
            'web' => 'nullable|string',
            // 'picture' => 'nullable|string',
            // 'tags' => 'required|array',
            // 'mini_template' => 'required|array',
            // 'sockets' => 'required|array',
            // 'input' => 'required|string',
            'inputs' => 'required|array',
            'inputs.*.name' => 'required|string',
            'inputs.*.description' => 'required|string',
            'inputs.*.required'=> 'required|boolean',
            'inputs.*.type' => 'required|string|in:string,integer,json,array',
            'output_type' => 'required|string|in:string,integer,json,array',
            'output_description' => 'required|string',
            'file_link' => ['required', 'string', 'url', 'active_url', new WasmUrl()],
        ];
    }


    public function submit()
    {

        // dd($this->inputs);

        $validated = $this->validate();

        // dd($validated);
        $good = false;

        try {

            $plugin = new Plugin();
            $plugin->kind = 5003;
            $plugin->name = $this->name;
            $plugin->description = $this->description;
            $plugin->tos = $this->tos;
            $plugin->privacy = $this->privacy;
            $plugin->author = $this->author;
            $plugin->web = $this->web;
            $plugin->picture = $this->picture;
            $plugin->tags = json_encode($this->tags);
            $plugin->mini_template = $this->generateMiniTemplate();
            $plugin->output_template = $this->generateOutputTemplate();
            $plugin->input_template =  $this->inputs->toJson();
            $plugin->user_id =  User::first()->id;
            // $plugin->authu
            $plugin->save();

            $good = true;
        } catch (\Throwable $th) {
            Log::error("error forom plugin : ".$th);
            $good = false;
        }


        if ($good) {
            $this->alert('success', 'Plugin successfully created.');
            return redirect()->route('plugins.index');
        } else {
            $this->alert('error', 'An error occured');
        }
    }


    public function generateOutputTemplate()
    {
        return json_encode([
            'type' => $this->output_type,
            'description' => $this->output_description,
        ]);
    }


    public function generateMiniTemplate()
    {
        return json_encode([
            'main' => $this->file_link,
            'input' => $this->generateInputMoustacheTemplate()
        ]);
    }

    public function generateInputMoustacheTemplate()
    {
        $template = '';

        foreach ($this->inputs as $input) {
            $template .= '{{in.' . $input['name'] . '}}';
            // if (isset($input['type']) && $input['type'] === 'string') {
            //     $template .= '|' . $input['name'];
            // }
            $template .= ' ';
        }

        return $template;
    }

    public function addInput()
    {
        $this->inputs->push([
            'name' => '',
            'required' => false,
            'type' => 'string',
            'description' => ''
        ]);
    }

    public function removeInput($key)
    {
        $this->inputs->pull($key);
    }



    public function render()
    {
        return view('livewire.plugins.plugin-create');
    }
}
