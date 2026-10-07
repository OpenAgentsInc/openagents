//! Native team policy reads and exact review through the selected customer.
use crate::{Args, Output};
use coder::customer::Store;
use receipts::team_policy::Change;
use std::path::Path;
pub const USAGE: &str = "usage: openagents customer policy COMMAND --root DIR [OPTIONS]
  read
        Read the selected native workspace policy; members see only its reference.
  review --input FILE
        Apply a private Change with expected_digest and exact versioned terms.

An owner reviews exact request and material digests, data classes, model or
plugin release, recipients, placement, and expiry. Administrators can narrow
existing rules. Unknown replies require read; a fresh intent cannot bypass the
native predecessor check. The first enabled lane is local Gateway SystemOne.
Cloud, customer-host, plugin, classification, and jobs are unavailable here.";
pub fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|s| matches!(s.as_str(), "--help" | "help" | "-h"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(words, &[]) {
        Ok(a) => a,
        Err(e) => return output.usage("customer policy", &e, USAGE),
    };
    let command = args
        .positional()
        .first()
        .filter(|_| args.positional().len() == 1)
        .map(String::as_str);
    let allowed = if command == Some("review") {
        vec!["root", "input"]
    } else {
        vec!["root"]
    };
    if !matches!(command, Some("read" | "review"))
        || allowed.iter().any(|n| args.option(n).is_none())
        || args.option_names().iter().any(|n| !allowed.contains(n))
    {
        return output.usage(
            "customer policy",
            "Select a declared command and private input.",
            USAGE,
        );
    }
    let result=crate::runtime().block_on(async {
        let store=Store::open(Path::new(args.option("root").unwrap()))?;
        let selected=store.selected().ok_or("Select a native customer and workspace first.")?;
        let client=store.client(&selected.origin,&selected.credential_alias)?;
        let account=&selected.context.account;let workspace=&selected.context.workspace;
        let view=if command==Some("review") {
            let change:Change=serde_json::from_slice(&Store::private_input(Path::new(args.option("input").unwrap()),128*1024)?).map_err(|_|"Invalid private team policy review.")?;
            client.account().review_team_policy(account,workspace,&change).await
        }else{client.account().team_policy(account,workspace).await}.map_err(|_|"Current native team policy rights or reviewed predecessor are unavailable; read before retrying.")?;
        serde_json::to_value(view).map_err(|_|"Invalid policy response.".to_owned())
    });
    match result {
        Ok(v) => {
            output.emit(&v, |v| serde_json::to_string(v).unwrap_or_default());
            0
        }
        Err(e) => output.fail("customer policy", &e),
    }
}
