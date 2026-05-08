use sqlx::{query, query_as, PgPool};

use crate::models::Post;

#[derive(Clone)]
#[allow(dead_code)]
pub struct PostRepository {
    db_pool: PgPool,
}

#[allow(dead_code)]
pub struct CreatePostParams<'a> {
    pub thread_id: i64,
    pub user_id: i64,
    pub body_md: &'a str,
    pub body_html: &'a str,
}

#[allow(dead_code)]
impl PostRepository {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    pub async fn create(&self, params: CreatePostParams<'_>) -> Result<Post, sqlx::Error> {
        query_as::<_, Post>(
            r#"
            INSERT INTO posts (thread_id, user_id, body_md, body_html)
            VALUES ($1, $2, $3, $4)
            RETURNING id, thread_id, user_id, body_md, body_html, edited_at, is_deleted, created_at
            "#,
        )
        .bind(params.thread_id)
        .bind(params.user_id)
        .bind(params.body_md)
        .bind(params.body_html)
        .fetch_one(&self.db_pool)
        .await
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Option<Post>, sqlx::Error> {
        query_as::<_, Post>(
            r#"
            SELECT id, thread_id, user_id, body_md, body_html, edited_at, is_deleted, created_at
            FROM posts
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.db_pool)
        .await
    }

    pub async fn list_by_thread(&self, thread_id: i64) -> Result<Vec<Post>, sqlx::Error> {
        query_as::<_, Post>(
            r#"
            SELECT id, thread_id, user_id, body_md, body_html, edited_at, is_deleted, created_at
            FROM posts
            WHERE thread_id = $1
            ORDER BY created_at ASC, id ASC
            "#,
        )
        .bind(thread_id)
        .fetch_all(&self.db_pool)
        .await
    }

    pub async fn mark_deleted(&self, id: i64) -> Result<(), sqlx::Error> {
        query(
            r#"
            UPDATE posts
            SET is_deleted = TRUE, edited_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.db_pool)
        .await
        .map(|_| ())
    }
}
