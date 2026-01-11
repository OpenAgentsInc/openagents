use crate::commands::{parse_command, Command};

#[test]
fn parse_spark_wallet_commands() {
    assert_eq!(parse_command("/spark"), Some(Command::SparkWallet));
    assert_eq!(parse_command("/spark refresh"), Some(Command::SparkWalletRefresh));
    assert_eq!(parse_command("/spark status"), Some(Command::SparkWallet));
}
