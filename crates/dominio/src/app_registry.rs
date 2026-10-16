//! Regras de autorização do App Registry, independentes de HTTP e PostgreSQL.
//! A participação é sempre consultada pelo par (app_id, conta) na camada de dados.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppRole {
    Owner,
    Editor,
    Viewer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    Read,
    EditDraft,
    CreateVersion,
    Publish,
    ManageMembers,
    ManageIntegrations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppMembership {
    pub app_id: Uuid,
    pub account_sub: String,
    pub role: AppRole,
}

impl AppRole {
    pub fn allows(self, action: AppAction) -> bool {
        match self {
            Self::Owner => true,
            Self::Editor => matches!(
                action,
                AppAction::Read | AppAction::EditDraft | AppAction::CreateVersion
            ),
            Self::Viewer => matches!(action, AppAction::Read),
        }
    }
}

/// A camada de dados deve consultar a participação usando app_id E account_sub.
/// A verificação abaixo protege também contra uso acidental de outra participação.
pub fn authorize(
    app_id: Uuid,
    account_sub: &str,
    membership: Option<&AppMembership>,
    action: AppAction,
) -> bool {
    membership.is_some_and(|member| {
        member.app_id == app_id
            && member.account_sub == account_sub
            && member.role.allows(action)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_can_manage_and_publish() {
        assert!(AppRole::Owner.allows(AppAction::Publish));
        assert!(AppRole::Owner.allows(AppAction::ManageMembers));
    }

    #[test]
    fn editor_cannot_publish_or_manage_members() {
        assert!(AppRole::Editor.allows(AppAction::EditDraft));
        assert!(!AppRole::Editor.allows(AppAction::Publish));
        assert!(!AppRole::Editor.allows(AppAction::ManageMembers));
    }

    #[test]
    fn viewer_can_only_read() {
        assert!(AppRole::Viewer.allows(AppAction::Read));
        assert!(!AppRole::Viewer.allows(AppAction::EditDraft));
    }

    #[test]
    fn rejects_other_app_or_account() {
        let app_id = Uuid::new_v4();
        let member = AppMembership {
            app_id,
            account_sub: "account-a".into(),
            role: AppRole::Owner,
        };
        assert!(authorize(app_id, "account-a", Some(&member), AppAction::Publish));
        assert!(!authorize(Uuid::new_v4(), "account-a", Some(&member), AppAction::Read));
        assert!(!authorize(app_id, "account-b", Some(&member), AppAction::Read));
        assert!(!authorize(app_id, "account-a", None, AppAction::Read));
    }
}
