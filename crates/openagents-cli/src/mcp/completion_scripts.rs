//! Shell completions projected from the bundled help tree.
use std::collections::BTreeMap;

use coder::cli_route::{
    tree::{self, Node},
    usage::Token,
};

struct Row {
    path: String,
    children: Vec<String>,
    flags: BTreeMap<String, bool>,
}

fn rows() -> Vec<Row> {
    fn visit(node: &Node, path: String, inherited: &BTreeMap<String, bool>, out: &mut Vec<Row>) {
        let mut flags = inherited.clone();
        for token in node.options.iter().chain(
            node.leaf
                .iter()
                .flat_map(|l| l.forms.iter().flat_map(|f| &f.tokens)),
        ) {
            if let Token::Option { name, value, .. } = token {
                if name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
                    flags.insert(format!("--{name}"), value.is_some());
                }
            }
        }
        out.push(Row {
            path: path.clone(),
            children: node.children.iter().map(|c| c.name.clone()).collect(),
            flags: flags.clone(),
        });
        for child in &node.children {
            visit(child, format!("{path}/{}", child.name), &flags, out);
        }
    }
    let flags = BTreeMap::from([("--json".into(), false), ("--help".into(), false)]);
    let mut out = vec![Row {
        path: String::new(),
        children: tree::bundled()
            .groups
            .iter()
            .map(|g| g.name.clone())
            .collect(),
        flags: flags.clone(),
    }];
    for group in &tree::bundled().groups {
        visit(group, format!("/{}", group.name), &flags, &mut out);
    }
    out
}

fn candidates(row: &Row) -> String {
    row.children
        .iter()
        .chain(row.flags.keys())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ")
}

// Both shells use the same walker. Arguments exclude the executable and
// the word being completed; option values must not become command words.
fn walker(rows: &[Row]) -> String {
    let mut script = String::from(
        "_openagents_candidates() {\n  local oa_path='' word skip=0\n  for word in \"$@\"; do\n    if [ \"$skip\" = 1 ]; then skip=0; continue; fi\n    [ \"$word\" = -- ] && return\n    case \"$oa_path:$word\" in\n",
    );
    for row in rows {
        for (flag, value) in &row.flags {
            if *value {
                script.push_str(&format!("      '{}:{flag}') skip=1 ;;\n", row.path));
            }
        }
        for child in &row.children {
            script.push_str(&format!(
                "      '{}:{child}') oa_path='{}/{child}' ;;\n",
                row.path, row.path
            ));
        }
    }
    script.push_str("    esac\n  done\n  [ \"$skip\" = 1 ] && return\n  case \"$oa_path\" in\n");
    for row in rows {
        script.push_str(&format!(
            "    '{}') printf '%s\\n' '{}' ;;\n",
            row.path,
            candidates(row)
        ));
    }
    script.push_str("  esac\n}\n");
    script
}

pub(super) fn bash() -> String {
    let mut script = String::from("# openagents bash completion: source this file.\n");
    script.push_str(&walker(&rows()));
    script.push_str("_openagents() {\n  local cur=${COMP_WORDS[COMP_CWORD]}\n  local choices=$(_openagents_candidates \"${COMP_WORDS[@]:1:COMP_CWORD-1}\")\n  COMPREPLY=( $(compgen -W \"$choices\" -- \"$cur\") )\n}\ncomplete -F _openagents openagents\n");
    script
}

pub(super) fn zsh() -> String {
    let mut script = String::from(
        "#compdef openagents\n# Source after compinit, or save on $fpath as _openagents.\n",
    );
    script.push_str(&walker(&rows()));
    script.push_str("_openagents() {\n  local -a candidates\n  candidates=(${=\"$(_openagents_candidates \"${(@)words[2,CURRENT-1]}\")\"})\n  compadd -- \"${candidates[@]}\"\n}\nif [[ $ZSH_EVAL_CONTEXT == *:file ]]; then\n  compdef _openagents openagents\nelse\n  _openagents \"$@\"\nfi\n");
    script
}

pub(super) fn fish() -> String {
    let rows = rows();
    let mut script = String::from(
        "# Source or save as ~/.config/fish/completions/openagents.fish.\nfunction __openagents_candidates\n  set -l words (commandline -opc)\n  set -e words[1]\n  set -l oa_path ''\n  set -l skip 0\n  for word in $words\n    if test $skip = 1\n      set skip 0\n      continue\n    end\n    if test \"$word\" = --\n      return\n    end\n    switch \"$oa_path:$word\"\n",
    );
    for row in &rows {
        for (flag, value) in &row.flags {
            if *value {
                script.push_str(&format!(
                    "      case '{}:{flag}'\n        set skip 1\n",
                    row.path
                ));
            }
        }
        for child in &row.children {
            script.push_str(&format!(
                "      case '{}:{child}'\n        set oa_path '{}/{child}'\n",
                row.path, row.path
            ));
        }
    }
    script.push_str("    end\n  end\n  if test $skip = 1\n    return\n  end\n  switch $oa_path\n");
    for row in &rows {
        script.push_str(&format!(
            "    case '{}'\n      printf '%s\\n' {}\n",
            row.path,
            candidates(row)
                .split_whitespace()
                .map(|s| format!("'{s}'"))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    script.push_str("  end\nend\ncomplete -c openagents -f -a '(__openagents_candidates)'\n");
    script
}
