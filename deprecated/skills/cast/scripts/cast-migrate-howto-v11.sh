#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_lib_receipts.sh"

input_file=""
output_spell_file=""
output_private_inputs_file=""
funding_utxo="${CAST_FUNDING_UTXO:-}"
receipt_file=""

usage() {
    cat <<EOF
Usage: $0 --input <spell.yaml> --output-spell <v11-spell.yaml> --output-private-inputs <private.yaml> [--funding-utxo <txid:vout>] [--receipt-file <path>]

Migrate a CAST howto spell from legacy format (version 9) to Charms v11 format.

Behavior:
  - converts apps/ins/outs/private_inputs into v11 tx/app_public_inputs + private-inputs file
  - converts each output address into tx.coins[*].dest using "charms util dest --addr"
  - auto-detects funding UTXO from comment line "# Funding UTXO: <txid:vout>" when present
  - appends funding UTXO into tx.ins if not already present
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
    --input)
        input_file="${2:-}"
        shift 2
        ;;
    --output-spell)
        output_spell_file="${2:-}"
        shift 2
        ;;
    --output-private-inputs)
        output_private_inputs_file="${2:-}"
        shift 2
        ;;
    --funding-utxo)
        funding_utxo="${2:-}"
        shift 2
        ;;
    --receipt-file)
        receipt_file="${2:-}"
        shift 2
        ;;
    -h | --help)
        usage
        exit 0
        ;;
    *)
        printf 'Unknown argument: %s\n' "$1" >&2
        usage >&2
        exit 1
        ;;
    esac
done

if [[ -z "$input_file" || -z "$output_spell_file" || -z "$output_private_inputs_file" ]]; then
    printf 'Missing required arguments.\n' >&2
    usage >&2
    exit 1
fi

if ! command -v ruby >/dev/null 2>&1; then
    printf 'Missing required command: ruby\n' >&2
    exit 1
fi
if ! command -v charms >/dev/null 2>&1; then
    printf 'Missing required command: charms\n' >&2
    exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
    printf 'Missing required command: jq\n' >&2
    exit 1
fi

if [[ ! -f "$input_file" ]]; then
    printf 'Input file not found: %s\n' "$input_file" >&2
    exit 1
fi

if [[ -z "$funding_utxo" ]]; then
    funding_utxo="$(sed -nE 's/^# Funding UTXO:[[:space:]]*([^[:space:]]+).*/\1/p' "$input_file" | head -n 1 || true)"
fi

mkdir -p "$(dirname "$output_spell_file")" "$(dirname "$output_private_inputs_file")"

migration_summary="$(ruby - "$input_file" "$output_spell_file" "$output_private_inputs_file" "$funding_utxo" <<'RUBY'
require 'json'
require 'open3'
require 'yaml'

input_path = ARGV.fetch(0)
output_spell_path = ARGV.fetch(1)
output_private_path = ARGV.fetch(2)
funding_utxo = ARGV.fetch(3).to_s.strip

def charms_dest_hex(addr)
  out, status = Open3.capture2('charms', 'util', 'dest', '--addr', addr)
  raise "charms util dest failed for #{addr}" unless status.success?
  out.strip
end

spell = YAML.load_file(input_path)
raise "spell must be a YAML mapping" unless spell.is_a?(Hash)

private_out = {}
funding_added = false

if spell['version'] == 11 && spell['tx'].is_a?(Hash)
  spell = Marshal.load(Marshal.dump(spell))
  private_out = spell.delete('private_inputs') || {}
  spell['app_public_inputs'] ||= {}

  ins = spell.dig('tx', 'ins') || []
  unless ins.is_a?(Array)
    raise 'tx.ins must be an array in v11 spell'
  end
  if !funding_utxo.empty? && !ins.include?(funding_utxo)
    ins << funding_utxo
    funding_added = true
  end
  spell['tx']['ins'] = ins
else
    apps = spell.fetch('apps')
    raise 'apps must be a mapping' unless apps.is_a?(Hash)
    legacy_private_inputs = spell['private_inputs'] || {}

  app_ids = apps.values.uniq.sort
  app_to_index = {}
  app_ids.each_with_index { |app_id, index| app_to_index[app_id] = index }

  ins = (spell['ins'] || []).map { |input| input.fetch('utxo_id') }
  if !funding_utxo.empty? && !ins.include?(funding_utxo)
    ins << funding_utxo
    funding_added = true
  end

  outs_src = spell['outs'] || []
  outs = outs_src.map do |out|
    charms = out['charms'] || {}
    mapped = {}
    charms.each do |alias_name, value|
      app_id = apps.fetch(alias_name)
      mapped[app_to_index.fetch(app_id)] = value
    end
    mapped
  end

  coins = outs_src.map do |out|
    address = out.fetch('address')
    {
      'amount' => (out.key?('coin') ? out['coin'] : 300),
      'dest' => charms_dest_hex(address),
    }
  end

  spell = {
    'version' => 11,
    'tx' => {
      'ins' => ins,
      'outs' => outs,
      'coins' => coins,
    },
    'app_public_inputs' => app_ids.each_with_object({}) { |app_id, acc| acc[app_id] = nil },
  }

  legacy_private_inputs.each do |alias_name, value|
    app_id = apps.fetch(alias_name)
    private_out[app_id] = value
  end
end

File.write(output_spell_path, YAML.dump(spell))
File.write(output_private_path, YAML.dump(private_out))

puts JSON.generate(
  input_file: input_path,
  output_spell_file: output_spell_path,
  output_private_inputs_file: output_private_path,
  version: spell['version'],
  input_count: (spell.dig('tx', 'ins') || []).length,
  output_count: (spell.dig('tx', 'outs') || []).length,
  app_count: (spell['app_public_inputs'] || {}).length,
  funding_utxo: funding_utxo,
  funding_added: funding_added
)
RUBY
)"

if ! jq empty <<<"$migration_summary" >/dev/null 2>&1; then
    printf 'Migration did not return valid JSON summary.\n' >&2
    exit 1
fi

result_json="$(jq -cn \
    --argjson summary "$migration_summary" \
    '{ok: true, operation: "migrate_howto_v11"} + $summary')"

cast_write_receipt "migrate_howto_v11" "$result_json" "$receipt_file"
