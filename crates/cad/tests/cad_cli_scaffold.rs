use openagents_cad::cli::{CAD_CLI_SCAFFOLD_COMMANDS, CAD_CLI_STUB_EXIT_CODE, run_cli_tokens};

#[test]
fn openagents_cad_cli_help_lists_surface_commands() {
    let outcome = run_cli_tokens(&["openagents-cad-cli", "--help"]);
    assert_eq!(outcome.exit_code, 0);
    for command in CAD_CLI_SCAFFOLD_COMMANDS {
        assert!(outcome.stdout.contains(command));
    }
}

#[test]
fn openagents_cad_cli_scaffold_commands_return_stub_message() {
    for command in CAD_CLI_SCAFFOLD_COMMANDS {
        let outcome = run_cli_tokens(&["openagents-cad-cli", command]);
        assert_eq!(outcome.exit_code, CAD_CLI_STUB_EXIT_CODE);
        assert!(
            outcome
                .stderr
                .contains("command scaffold is present; implementation lands in VCAD-PARITY-084")
        );
    }
}
