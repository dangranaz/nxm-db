<div align="center">

# nxm-db

<!-- Licensed MIT — part of the nxm tooling. -->
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![SQLite](https://img.shields.io/badge/SQLite-supported-003B57?logo=sqlite&logoColor=white)](https://www.sqlite.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-supported-4169E1?logo=postgresql&logoColor=white)](https://www.postgresql.org/)
[![MongoDB](https://img.shields.io/badge/MongoDB-supported-4DB33D?logo=mongodb&logoColor=white)](https://www.mongodb.com/)
[![Version](https://img.shields.io/badge/version-0.3.0-blue)](CHANGELOG.md)

**Datasource auto-configuration for NXM MCP servers**

<br>

<picture>
  <source srcset="docs/assets/nexum-banner.webp" type="image/webp">
  <img src="docs/assets/nexum-banner.png" alt="Nexum-AI banner" width="1000" height="553">
</picture>

<br>

`nxm-db` is a datasource auto-configuration brick for NXM MCP servers, in the style of a Spring Boot starter: you provide a configuration and get a ready-to-use backend (SQLite) + connection pool + migration runner + tenant. From local to multi-tenant, the change is configuration, not code.

It is not an ORM, not a query builder, not an entity abstraction: products own their own schema ("schema di competenza") with their own repositories and backend implementation. This crate provides only the runtime and configuration they rely on.

SQLite is the only backend in the public edition.

</div>

## What it provides

| Element | Description |
|---------|-------------|
| `Backend` | Selected backend; in the public edition only `Backend::Sqlite`. |
| `DbConfig` | `backend`, `url` (SQLite file path, `~` expanded) and `tenant` (default `"default"`). |
| `DbConfigSource` | Port implemented by the product to resolve a `DbConfig`. |
| `open` / `open_from` | Open the backend from a `DbConfig` or a `DbConfigSource`. |
| `Db::sqlite_pool` | Returns the SQLite pool (`r2d2` + `r2d2_sqlite`). |
| `Db::migrate` | Applies a `MigrationSet` to the opened backend. |
| `run_migrations` | Pure, backend-agnostic runner driven by `MigrationBackend`. |
| `DbError` / `Result` | Single error type of the crate (`Config`, `Connection`, `Migration`, ...). |

Migrations: `Migration` (monotonic version, name, SQL `up`), `MigrationSet` (validates unique and strictly increasing versions, no silent reordering) and `run_migrations`, which skips already-applied versions and applies the others in order. For SQLite, each migration runs in its own transaction with a versioning table created on first use.

## Quick example

```rust
use nxm_db::{open_from, DbConfig, DbConfigSource, Migration, MigrationSet, Result};

struct Config;

impl DbConfigSource for Config {
    fn db_config(&self) -> Result<DbConfig> {
        Ok(DbConfig::default())
    }
}

fn main() -> Result<()> {
    let db = open_from(&Config)?;

    let set = MigrationSet::new(vec![Migration::new(
        1,
        "create_users",
        "CREATE TABLE users (id INTEGER PRIMARY KEY);",
    )])?;
    db.migrate(&set)?;

    let pool = db.sqlite_pool()?;
    let _conn = pool.get().expect("pool has a connection");
    Ok(())
}
```

## Configuration

The `db` section lives inside the server config,
`~/.nxmconfig/<name>_config.json`, in canonical JSON format (TOML is accepted).
The file is loaded by `nxm_mcp_config`; the section is extracted with
`DbConfig::from_mcp`.

```json
{
  "db": {
    "backend": "sqlite",
    "url": "~/.nxm/shared-db/data.db",
    "tenant": "default"
  }
}
```

Equivalent TOML:

```toml
[db]
backend = "sqlite"
url = "~/.nxm/shared-db/data.db"
tenant = "default"
```

```rust
use nxm_db::DbConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mcp_config = nxm_mcp_config::load_mcp_config()?;
    let config = DbConfig::from_mcp(&mcp_config)?;
    let db = nxm_db::open(&config)?;
    // ...
    Ok(())
}
```

`tenant` is present from day one: the move to multi-tenant is configuration, not rewrite. `DbConfig::default()` is the local zero-config case (`~/.nxm/shared-db/data.db`, tenant `default`); if you already extracted the section you can use `DbConfig::from_toml(toml::Value)`.

## Logging

The crate emits `tracing` events on targets `nxm_db::runtime`, `nxm_db::sqlite`
and `nxm_db::migrate` (opening, pool usage, migration application).

Subscriber initialisation is always up to the server, via `nxm_mcp_logs`:
`nxm-db` never installs a global subscriber. Without init, events are simply
discarded, as per the `tracing` contract.

```rust
use nxm_mcp_logs::init_from_env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_from_env()?; // the server initialises; nxm-db never does
    // ...
    Ok(())
}
```

## Installation

```toml
# Cargo.toml
[dependencies]
nxm-db = { path = "../shared/nxm-db" }
```

Or from crates.io when published:

```toml
nxm-db = "0.2"
```

## Tests

```bash
cargo test
```

## License

Open source, **MIT** license ([LICENSE](LICENSE)).
