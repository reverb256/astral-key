//! Astral Key - OIDC group membership
//!
//! Supplies the `groups` claim for id_tokens and /oidc/userinfo.

use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use uuid::Uuid;

use crate::error::Result;

/// Group names for a user, ordered by name.
///
/// Always returns a vector — empty when the user has no membership — so
/// callers can emit the claim as an array unconditionally.
pub async fn groups_for_user(pool: &SqlitePool, user_id: Uuid) -> Result<Vec<String>> {
    let rows = sqlx::query(
        "SELECT group_name FROM oidc_group_members WHERE user_id = ?1 ORDER BY group_name",
    )
    .bind(user_id.to_string())
    .fetch_all(pool)
    .await?;

    let mut groups = Vec::with_capacity(rows.len());
    for row in rows {
        let name: &str = row.get("group_name");
        groups.push(name.to_string());
    }

    Ok(groups)
}
