use chrono::{DateTime, Utc};
use sqlx::{query, query_as, PgPool};

use crate::models::Thread;

#[derive(Clone)]
#[allow(dead_code)]
pub struct ThreadRepository {
    db_pool: PgPool,
}

#[allow(dead_code)]
pub struct CreateThreadParams<'a> {
    pub category_id: i64,
    pub user_id: i64,
    pub title: &'a str,
    pub slug: &'a str,
}

#[allow(dead_code)]
impl ThreadRepository {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    pub async fn create(&self, params: CreateThreadParams<'_>) -> Result<Thread, sqlx::Error> {
        query_as::<_, Thread>(
            r#"
            INSERT INTO threads (category_id, user_id, title, slug)
            VALUES ($1, $2, $3, $4)
            RETURNING
                id,
                category_id,
                user_id,
                title,
                slug,
                is_pinned,
                is_locked,
                is_deleted,
                created_at,
                last_activity_at
            "#,
        )
        .bind(params.category_id)
        .bind(params.user_id)
        .bind(params.title)
        .bind(params.slug)
        .fetch_one(&self.db_pool)
        .await
    }

    pub async fn find_by_category_and_slug(
        &self,
        category_id: i64,
        slug: &str,
    ) -> Result<Option<Thread>, sqlx::Error> {
        query_as::<_, Thread>(
            r#"
            SELECT
                id,
                category_id,
                user_id,
                title,
                slug,
                is_pinned,
                is_locked,
                is_deleted,
                created_at,
                last_activity_at
            FROM threads
            WHERE category_id = $1 AND slug = $2
            "#,
        )
        .bind(category_id)
        .bind(slug)
        .fetch_optional(&self.db_pool)
        .await
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Option<Thread>, sqlx::Error> {
        query_as::<_, Thread>(
            r#"
            SELECT
                id,
                category_id,
                user_id,
                title,
                slug,
                is_pinned,
                is_locked,
                is_deleted,
                created_at,
                last_activity_at
            FROM threads
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.db_pool)
        .await
    }

    pub async fn list_by_category(&self, category_id: i64) -> Result<Vec<Thread>, sqlx::Error> {
        query_as::<_, Thread>(
            r#"
            SELECT
                id,
                category_id,
                user_id,
                title,
                slug,
                is_pinned,
                is_locked,
                is_deleted,
                created_at,
                last_activity_at
            FROM threads
            WHERE category_id = $1
            ORDER BY is_pinned DESC, last_activity_at DESC, id DESC
            "#,
        )
        .bind(category_id)
        .fetch_all(&self.db_pool)
        .await
    }

    pub async fn touch_last_activity(
        &self,
        id: i64,
        last_activity_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        query(
            r#"
            UPDATE threads
            SET last_activity_at = $2
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(last_activity_at)
        .execute(&self.db_pool)
        .await
        .map(|_| ())
    }
}
