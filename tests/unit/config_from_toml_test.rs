use nxm_db::{Backend, DbConfig, DbError};

// --- Helper per costruire toml::Value da una tabella inline ---
//
// Il valore rappresenta il contenuto diretto della sezione [db] (senza
// l'intestazione [db]), così com'è passato a DbConfig::from_toml.

fn toml_table(pairs: &[(&str, &str)]) -> toml::Value {
    let mut s = String::new();
    for (k, v) in pairs {
        s.push_str(&format!("{k} = {v}\n"));
    }
    toml::from_str(&s).expect("toml table is valid")
}

fn toml_str(v: &str) -> String {
    format!("\"{v}\"")
}

// --- Test 1: parsing di una sezione [db] valida ---

#[test]
fn from_toml_parses_valid_section() {
    let value = toml_table(&[
        ("backend", &toml_str("sqlite")),
        ("url", &toml_str("~/.nxm/shared-db/data.db")),
        ("tenant", &toml_str("acme")),
    ]);

    let config = DbConfig::from_toml(value).expect("valid section should parse");

    assert_eq!(config.backend, Backend::Sqlite);
    assert_eq!(config.url, "~/.nxm/shared-db/data.db");
    assert_eq!(config.tenant, "acme");
}

// --- Test 2: tenant assente → default "default" ---

#[test]
fn from_toml_defaults_tenant() {
    let value = toml_table(&[
        ("backend", &toml_str("sqlite")),
        ("url", &toml_str("~/.nxm/shared-db/data.db")),
    ]);

    let config = DbConfig::from_toml(value).expect("section without tenant should parse");

    assert_eq!(config.backend, Backend::Sqlite);
    assert_eq!(config.url, "~/.nxm/shared-db/data.db");
    assert_eq!(config.tenant, "default");
}

// --- Test 3: backend ignoto → errore tipato, niente panic ---

#[test]
fn from_toml_rejects_unknown_backend() {
    let value = toml_table(&[
        ("backend", &toml_str("postgres")),
        ("url", &toml_str("postgres://host/db")),
    ]);

    let result = DbConfig::from_toml(value);

    assert!(
        result.is_err(),
        "unknown backend must return an error, not panic"
    );
    match result {
        Err(DbError::Config(msg)) => {
            assert!(!msg.is_empty(), "error message must not be empty");
        }
        Err(other) => panic!("expected DbError::Config, got {:?}", other),
        Ok(_) => panic!("expected an error for unknown backend"),
    }
}

// --- Test 4: campo obbligatorio mancante (url) → errore tipato ---

#[test]
fn from_toml_requires_mandatory_fields() {
    let value = toml_table(&[("backend", &toml_str("sqlite"))]);

    let result = DbConfig::from_toml(value);

    assert!(
        result.is_err(),
        "missing mandatory url must return an error"
    );
    match result {
        Err(DbError::Config(_)) => {}
        Err(other) => panic!("expected DbError::Config, got {:?}", other),
        Ok(_) => panic!("expected an error for missing mandatory field"),
    }
}

// --- Test 5: round-trip di default ---

#[test]
fn from_toml_roundtrips_default() {
    let original = DbConfig::default();
    let toml_string = toml::to_string(&original).expect("default config serializes to toml");
    let value: toml::Value = toml::from_str(&toml_string).expect("toml string parses back");
    let roundtripped = DbConfig::from_toml(value).expect("round-trip should succeed");

    assert_eq!(roundtripped, original);
}
