//! Cache-first đọc membership nhóm (id + role), dùng chung cho authorize ở mọi domain.
//!
//! Tiết kiệm 1–2 round trip DB mỗi request: hầu hết endpoint đều check membership qua
//! `find_members` full-row, trong khi authorize chỉ cần id + role. Trade-off: staleness
//! tối đa bằng TTL nếu sót điểm invalidate — mọi điểm mutate (add member, delete group)
//! đều phải gọi [`invalidate_member_cache`].

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    domain::{GroupRole, groups::repository::GroupRepository},
    errors::{AppError, BusinessError},
    utils::cache::{CACHE_EXPIRATION, Cache, CacheStore, CacheStoreExt},
};

/// Membership rút gọn lưu cache — đủ cho authorize, nhẹ hơn full row `GroupMemberWithUser`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberEntry {
    pub user_id: Uuid,
    pub role: GroupRole,
}

fn cache_key(group_id: Uuid) -> String {
    format!("group:members:{group_id}")
}

/// Lấy membership nhóm, ưu tiên cache; miss thì query rồi nạp lại cache.
///
/// # Errors
///
/// Trả lỗi DB khi query thất bại (cache miss/lỗi cache không bao giờ fail).
pub async fn member_entries<GR: GroupRepository>(
    cache: &Cache,
    pool: &PgPool,
    repo: &GR,
    group_id: Uuid,
) -> Result<Vec<MemberEntry>, AppError> {
    let key = cache_key(group_id);
    if let Some(cached) = cache.get::<Vec<MemberEntry>>(&key).await {
        return Ok(cached);
    }
    let members = repo.find_members(pool, group_id).await?;
    let entries: Vec<MemberEntry> = members.iter().map(|m| MemberEntry { user_id: m.user_id, role: m.role }).collect();
    cache.set(&key, &entries, CACHE_EXPIRATION).await;
    Ok(entries)
}

/// Xóa cache membership — gọi ở mọi điểm mutate members (add/remove member, delete group).
pub async fn invalidate_member_cache(cache: &Cache, group_id: Uuid) {
    cache.delete(&cache_key(group_id)).await;
}

/// Chặn nếu user không phải thành viên nhóm (bản cache-first).
///
/// # Errors
///
/// Trả `NotGroupMember` khi user ngoài nhóm.
pub async fn ensure_membership<GR: GroupRepository>(
    cache: &Cache,
    pool: &PgPool,
    repo: &GR,
    group_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let members = member_entries(cache, pool, repo, group_id).await?;
    if !members.iter().any(|m| m.user_id == user_id) {
        return Err(AppError::Business(BusinessError::NotGroupMember));
    }
    Ok(())
}

/// Chặn nếu user không phải admin nhóm (bản cache-first).
///
/// # Errors
///
/// Trả `AdminRequired` khi user ngoài nhóm hoặc không phải admin.
pub async fn ensure_admin<GR: GroupRepository>(
    cache: &Cache,
    pool: &PgPool,
    repo: &GR,
    group_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let members = member_entries(cache, pool, repo, group_id).await?;
    let member = members.iter().find(|m| m.user_id == user_id);
    match member {
        Some(m) if m.role == GroupRole::ADMIN => Ok(()),
        _ => Err(AppError::Business(BusinessError::AdminRequired)),
    }
}
