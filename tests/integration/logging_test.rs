use nxm_db::{open_from, Backend, DbConfig, DbConfigSource, Result};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::writer::MakeWriterExt;

// --- Capturing writer for tracing output in tests ---

struct BufferWriter(Arc<Mutex<Vec<u8>>>);

impl<'a> tracing_subscriber::fmt::writer::MakeWriter<'a> for BufferWriter {
    type Writer = BufferWriter;

    fn make_writer(&self) -> Self::Writer {
        BufferWriter(self.0.clone())
    }
}

impl Write for BufferWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// --- Fake source implementing the DbConfigSource port ---

struct FakeSource {
    config: Result<DbConfig>,
}

impl FakeSource {
    fn new(config: Result<DbConfig>) -> Self {
        Self { config }
    }
}

impl DbConfigSource for FakeSource {
    fn db_config(&self) -> Result<DbConfig> {
        self.config.clone()
    }
}

fn tmp_db_path() -> String {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir()
        .join(format!(
            "nxm_db-logging-{}-{nonce}/data.db",
            std::process::id()
        ))
        .to_string_lossy()
        .into_owned()
}

// --- Test: open_from emits ENTER/EXIT on the per-feature target ---
//
// Questo test gira nel proprio processo di test (ogni file sotto tests/ è
// un binario separato), quindi non c'è race con altri test sul dispatch
// thread-local di `tracing` (`with_default`). È deterministico sotto
// parallelismo di default.

#[test]
fn open_from_logs_enter_exit() {
    let buffer = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = BufferWriter(buffer.clone()).and(std::io::stderr);

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new("nxm_db=debug"))
        .with_writer(writer)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        let config = DbConfig {
            backend: Backend::Sqlite,
            url: tmp_db_path(),
            tenant: "default".to_string(),
        };
        let source = FakeSource::new(Ok(config));
        let _result = open_from(&source);
    });

    let guard = buffer.lock().unwrap();
    let output = String::from_utf8_lossy(&guard);
    assert!(
        output.contains("ENTER"),
        "missing ENTER log in output:\n{output}"
    );
    assert!(
        output.contains("EXIT"),
        "missing EXIT log in output:\n{output}"
    );
}
