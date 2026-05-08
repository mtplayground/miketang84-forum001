use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
#[allow(dead_code)]
pub enum UserRole {
    #[default]
    User,
    Admin,
}

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub role: UserRole,
    pub bio: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Category {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub position: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Thread {
    pub id: i64,
    pub category_id: i64,
    pub user_id: i64,
    pub title: String,
    pub slug: String,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub is_deleted: bool,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Post {
    pub id: i64,
    pub thread_id: i64,
    pub user_id: i64,
    pub body_md: String,
    pub body_html: String,
    pub edited_at: Option<DateTime<Utc>>,
    pub is_deleted: bool,
    pub created_at: DateTime<Utc>,
}
