# nxm_db

Datasource auto-configuration per i server MCP del progetto.

## Scopo

`nxm_db` è un brick di auto-configurazione del datasource, nello spirito di uno
starter Spring Boot: gli si dà una config e restituisce un backend pronto
(SQLite) + pool di connessioni + runner di migrazioni + tenant. Da locale a
multi-tenant il passaggio è una modifica di configurazione, non di codice.

Non è un ORM, non è un query builder, non è un'astrazione di entità: i prodotti
possiedono il proprio schema ("schema di competenza") con i propri repository e
la propria implementazione di backend. Questa crate fornisce solo il runtime e
la configurazione su cui appoggiano.

SQLite è l'unico backend.

## Cosa fornisce

| Elemento | Descrizione |
|----------|-------------|
| `Backend` | Backend selezionato; nell'edizione pubblica solo `Backend::Sqlite`. |
| `DbConfig` | `backend`, `url` (path del file SQLite, `~` espanso) e `tenant` (default `"default"`). |
| `DbConfigSource` | Porta implementata dal prodotto per risolvere un `DbConfig`. |
| `open` / `open_from` | Aprono il backend da un `DbConfig` o da una `DbConfigSource`. |
| `Db::sqlite_pool` | Restituisce il pool SQLite (`r2d2` + `r2d2_sqlite`). |
| `Db::migrate` | Applica un `MigrationSet` al backend aperto. |
| `run_migrations` | Runner puro, backend-agnostico, guidato da `MigrationBackend`. |
| `DbError` / `Result` | Errore unico della crate (`Config`, `Connection`, `Migration`, ...). |

Migrazioni: `Migration` (versione monotona, nome, SQL `up`), `MigrationSet`
(valida versioni uniche e strettamente crescenti, senza riordini silenziosi) e
`run_migrations`, che salta le versioni già applicate e applica le altre in
ordine. Per SQLite ogni migrazione gira nella propria transazione con tabella di
versioning creata al primo uso.

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

## Config

La sezione `db` vive dentro la config del server,
`~/.nxmconfig/<nome>_config.json`, nel formato JSON canonico (TOML è accettato).
Il file è caricato da `nxm_mcp_config`; la sezione si estrae con
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

Equivalente TOML:

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

`tenant` è presente fin dal primo giorno: il passaggio a multi-tenant è
configurazione, non riscrittura. `DbConfig::default()` è il caso locale
zero-config (`~/.nxm/shared-db/data.db`, tenant `default`); chi ha già estratto
la sezione può usare `DbConfig::from_toml(toml::Value)`.

## Logging

La crate emette eventi `tracing` sui target `nxm_db::runtime`, `nxm_db::sqlite`
e `nxm_db::migrate` (apertura, uso del pool, applicazione delle migrazioni).

L'inizializzazione del subscriber spetta sempre al server, via `nxm_mcp_logs`:
`nxm_db` non installa mai un subscriber globale. Senza init gli eventi vengono
semplicemente scartati, come da contratto `tracing`.

```rust
use nxm_mcp_logs::init_from_env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_from_env()?; // il server inizializza; nxm_db non lo fa mai
    // ...
    Ok(())
}
```

## Verifica

```bash
cargo test
```
