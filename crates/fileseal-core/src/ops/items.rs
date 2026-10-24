//! Removing items from a database, and putting them back: a file deleted on
//! purpose, a folder added by mistake, a disk given away. Removed items are
//! no longer used to verify or to search, but nothing is erased (cadrage,
//! R8: explicit, traced, history kept). Files and hash files are never
//! touched.

use serde::Serialize;
use ts_rs::TS;

use crate::problem::Problem;
use crate::store::{OperationKind, OperationStatus, Store};

/// What a removal or a putting back did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ItemsChanged {
    pub changed: u32,
    /// Putting back: places another item holds now (those stay removed).
    pub taken: Vec<String>,
}

/// Removes these items from the database, as one operation.
pub fn remove(store: &mut Store, item_ids: &[i64]) -> Result<ItemsChanged, Problem> {
    let operation = store.begin_operation(OperationKind::Attach)?;
    let removed = store.retire_items(item_ids);
    let summary = serde_json::json!({ "remove": removed.as_ref().ok() });
    finish(store, operation, removed.is_ok(), &summary);
    let removed = removed?;
    tracing::info!(removed, "items removed from the database");
    Ok(ItemsChanged {
        changed: removed,
        taken: Vec::new(),
    })
}

/// Puts removed items back where they were, as one operation.
pub fn put_back(store: &mut Store, item_ids: &[i64]) -> Result<ItemsChanged, Problem> {
    let operation = store.begin_operation(OperationKind::Attach)?;
    let restored = store.restore_items(item_ids);
    let summary = serde_json::json!({
        "putBack": restored.as_ref().ok().map(|r| r.restored),
        "taken": restored.as_ref().ok().map(|r| r.taken.len()),
    });
    finish(store, operation, restored.is_ok(), &summary);
    let restored = restored?;
    tracing::info!(
        restored = restored.restored,
        taken = restored.taken.len(),
        "items put back in the database"
    );
    Ok(ItemsChanged {
        changed: restored.restored,
        taken: restored.taken,
    })
}

/// The schema v1 is frozen and its operation kinds fixed: like reassigning
/// a folder, this is recorded as an `attach`, what it did in its summary.
fn finish(store: &mut Store, operation: i64, ok: bool, summary: &serde_json::Value) {
    let status = if ok {
        OperationStatus::Completed
    } else {
        OperationStatus::Failed
    };
    if let Err(problem) = store.finish_operation(operation, status, Some(&summary.to_string())) {
        tracing::warn!(%problem, "operation not closed in the database");
    }
}

#[cfg(test)]
mod tests;
