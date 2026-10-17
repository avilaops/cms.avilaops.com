//! Contrato declarativo para apps. Independente do motor de sites e de runtimes nativos.
//! AAD v1: somente navegação e componentes determinísticos, sem código remoto.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const SCHEMA_VERSION: &str = "1.0.0";
pub const MAX_SCREENS: usize = 100;
pub const MAX_COMPONENTS_PER_SCREEN: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppDefinition {
    pub schema_version: String,
    pub app_id: String,
    pub minimum_runtime_version: String,
    pub navigation: Navigation,
    pub screens: Vec<Screen>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Navigation {
    pub initial_route: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screen {
    pub id: String,
    pub route: String,
    pub title: String,
    pub components: Vec<Component>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Component {
    Text {
        id: String,
        value: String,
    },
    Button {
        id: String,
        label: String,
        action: Action,
    },
    Spacer {
        id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Action {
    Navigate { target: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    #[error("versão de schema não suportada")]
    UnsupportedSchema,
    #[error("identificador inválido")]
    InvalidIdentifier,
    #[error("limite de telas ou componentes excedido")]
    LimitExceeded,
    #[error("rota ou identificador duplicado")]
    Duplicate,
    #[error("rota inicial ou destino da navegação inexistente")]
    UnknownRoute,
}

fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id.as_bytes()[0].is_ascii_lowercase()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

impl Component {
    fn id(&self) -> &str {
        match self {
            Self::Text { id, .. } | Self::Button { id, .. } | Self::Spacer { id } => id,
        }
    }
}

impl AppDefinition {
    pub fn validate(&self) -> Result<(), ValidationError> {
        use ValidationError as E;
        if self.schema_version != SCHEMA_VERSION {
            return Err(E::UnsupportedSchema);
        }
        if !valid_id(&self.app_id) || self.minimum_runtime_version != SCHEMA_VERSION {
            return Err(E::InvalidIdentifier);
        }
        if self.screens.is_empty() || self.screens.len() > MAX_SCREENS {
            return Err(E::LimitExceeded);
        }
        let mut ids = HashSet::new();
        let mut routes = HashSet::new();
        for screen in &self.screens {
            if !valid_id(&screen.id) || !valid_id(&screen.route) {
                return Err(E::InvalidIdentifier);
            }
            if !ids.insert(&screen.id) || !routes.insert(&screen.route) {
                return Err(E::Duplicate);
            }
            if screen.components.len() > MAX_COMPONENTS_PER_SCREEN {
                return Err(E::LimitExceeded);
            }
            let mut component_ids = HashSet::new();
            for component in &screen.components {
                if !valid_id(component.id()) {
                    return Err(E::InvalidIdentifier);
                }
                if !component_ids.insert(component.id()) {
                    return Err(E::Duplicate);
                }
            }
        }
        if !routes.contains(&self.navigation.initial_route) {
            return Err(E::UnknownRoute);
        }
        for screen in &self.screens {
            for component in &screen.components {
                if let Component::Button {
                    action: Action::Navigate { target },
                    ..
                } = component
                {
                    if !routes.contains(target) {
                        return Err(E::UnknownRoute);
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> AppDefinition {
        serde_json::from_str(include_str!("../../../examples/aad/app-minimo.json"))
            .expect("fixture deve ser válida")
    }

    #[test]
    fn accepts_minimal_definition() {
        assert_eq!(fixture().validate(), Ok(()));
    }

    #[test]
    fn rejects_unknown_route() {
        let mut app = fixture();
        app.navigation.initial_route = "inexistente".into();
        assert_eq!(app.validate(), Err(ValidationError::UnknownRoute));
    }

    #[test]
    fn rejects_duplicate_component_id() {
        let mut app = fixture();
        app.screens[0]
            .components
            .push(Component::Spacer { id: "intro".into() });
        assert_eq!(app.validate(), Err(ValidationError::Duplicate));
    }

    #[test]
    fn rejects_duplicate_screen_route() {
        let mut app = fixture();
        app.screens[1].route = "home".into();
        assert_eq!(app.validate(), Err(ValidationError::Duplicate));
    }

    #[test]
    fn rejects_button_target_without_screen() {
        let mut app = fixture();
        app.screens[1].route = "changed".into();
        assert_eq!(app.validate(), Err(ValidationError::UnknownRoute));
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let mut app = fixture();
        app.schema_version = "2.0.0".into();
        assert_eq!(app.validate(), Err(ValidationError::UnsupportedSchema));
    }

    #[test]
    fn rejects_empty_identifier() {
        let mut app = fixture();
        app.app_id.clear();
        assert_eq!(app.validate(), Err(ValidationError::InvalidIdentifier));
    }

    #[test]
    fn rejects_unknown_fields() {
        let json = r#"{"schemaVersion":"1.0.0","appId":"x","minimumRuntimeVersion":"1.0.0","navigation":{"initialRoute":"home"},"screens":[],"script":"evil"}"#;
        assert!(serde_json::from_str::<AppDefinition>(json).is_err());
    }
}
