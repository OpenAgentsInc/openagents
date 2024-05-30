<?php

namespace App\Http\Resources;

use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class PluginResource extends JsonResource
{

        /**
     * The "data" wrapper that should be applied.
     *
     * @var string
     */
    public static $wrap = null;


    /**
     * Transform the resource into an array.
     *
     * @return array<string, mixed>
     */
    public function toArray(Request $request): array
    {
        // return parent::toArray($request);

        // Decode the input and output templates
        $inputTemplate = json_decode($this->input_template, true);
        $outputTemplate = json_decode($this->output_template, true);

        // Prepare the sockets data
        $sockets = [
            'in' => [],
            'out' => []
        ];

        // Populate the 'in' sockets using the input_template
        if (is_array($inputTemplate)) {
            foreach ($inputTemplate as $input) {
                $sockets['in'][$input['name']] = [
                    'type' => $input['type'],
                    'description' => $input['description'],
                    'required' => $input['required'],
                ];
            }
        }

          // Populate the 'out' sockets using the output_template
          if (is_array($outputTemplate)) {
            $sockets['out']['output'] = [
                'type' => $outputTemplate['type'],
                'description' => $outputTemplate['description'],
            ];
        }

        return [
            "meta" => [
                "kind" => (int) $this->kind,
                "name" => $this->name,
                "description" => $this->description,
                "tos" => $this->tos,
                "privacy" => $this->privacy,
                "author" => $this->author ? $this->author : $this->user->name,
                "web" =>$this->web,
                "picture" => $this->picture ?? '',
                "tags" => ["tool"],
                "payment" => "lightning:".$this->payment ? $this->payment : $this->user->lightning_address,
            ],
            "mini-template" => [
                "main" => $this->file_link,
                "input" => $this->plugin_input,
            ],
            "sockets" => $sockets
        ];
    }
}
