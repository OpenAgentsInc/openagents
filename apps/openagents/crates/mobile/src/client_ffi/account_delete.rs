//! Delete account in Settings (#12002, App Store guideline 5.1.1(v)).
//! Swift shows the warning and the confirmation; the request, its
//! refusals, and signing out afterwards live here, through the same
//! account controller as the desktop.
use super::CoreAccount;
use openagents_account::Deletion;

/// What a delete request came to.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AccountDeletion {
    /// The account is gone and the app is signed out.
    Deleted,
    /// Paid credit is left: ask, then call again with `forfeit_balance`.
    PaidCredit { message: String },
    /// The person owns a team with other members.
    SharedTeam { message: String },
    /// It didn't work; the message says what to do.
    Failed { message: String },
}

/// The words Settings shows before the person deletes the account: what
/// goes, and that the wallet's 12 words still hold the wallet.
#[uniffi::export]
pub fn account_deletion_warning() -> String {
    openagents_account::DELETE_WARNING.into()
}

#[uniffi::export]
impl CoreAccount {
    /// Delete the signed-in account and sign this app out. Call it only
    /// after the person confirmed the warning; with `forfeit_balance`,
    /// only after they confirmed giving up paid credit.
    pub async fn delete_account(&self, forfeit_balance: bool) -> AccountDeletion {
        let controller = self.controller().clone();
        let task = openagents_client::runtime::shared()
            .spawn(async move { controller.delete_account(forfeit_balance).await });
        match task.await {
            Ok(Deletion::Deleted) => AccountDeletion::Deleted,
            Ok(Deletion::PaidCredit) => AccountDeletion::PaidCredit {
                message: openagents_account::PAID_CREDIT_WARNING.into(),
            },
            Ok(Deletion::SharedTeam) => AccountDeletion::SharedTeam {
                message: openagents_account::SHARED_TEAM_NOTICE.into(),
            },
            Ok(Deletion::Failed(message)) => AccountDeletion::Failed { message },
            Err(_) => AccountDeletion::Failed {
                message: "We couldn't delete your account. Try again.".into(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_warning_keeps_the_wallet_words_in_view() {
        let warning = super::account_deletion_warning();
        assert!(warning.contains("12 words"));
        assert!(warning.contains("can't be undone"));
    }
}
