pub mod kv;
mod log_store;
mod state_machine;

use log_store::RocksLogStore;
use openraft::RaftTypeConfig;
use rocksdb::ColumnFamilyDescriptor;
use rocksdb::DB;
use rocksdb::Options;
pub use state_machine::RocksStateMachine;
use std::io;
use std::path::Path;
use std::sync::Arc;

/// Create a pair of `RocksLogStore` and `RocksStateMachine` that are backed by a same rocks db
/// instance.
pub async fn create_rocks_stores<C, P: AsRef<Path>>(
    db_path: P,
) -> Result<(RocksLogStore<C>, RocksStateMachine), io::Error>
where
    C: RaftTypeConfig,
{
    let mut db_opts = Options::default();
    db_opts.create_missing_column_families(true);
    db_opts.create_if_missing(true);

    let meta = ColumnFamilyDescriptor::new("meta", Options::default());
    let sm_meta = ColumnFamilyDescriptor::new("sm_meta", Options::default());
    let sm_data = ColumnFamilyDescriptor::new("sm_data", Options::default());
    let logs = ColumnFamilyDescriptor::new("logs", Options::default());

    let db_path = db_path.as_ref();
    let snapshot_dir = db_path.join("snapshots");

    let db = DB::open_cf_descriptors(&db_opts, db_path, vec![meta, sm_meta, sm_data, logs])
        .map_err(io::Error::other)?;

    let db = Arc::new(db);
    Ok((
        RocksLogStore::new(db.clone()),
        RocksStateMachine::new(db, snapshot_dir).await?,
    ))
}
