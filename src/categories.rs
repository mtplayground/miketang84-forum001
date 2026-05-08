use sqlx::{query_as, PgPool};

use crate::models::Category;

#[derive(Clone)]
#[allow(dead_code)]
pub struct CategoryRepository {
    db_pool: PgPool,
}

#[allow(dead_code)]
impl CategoryRepository {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    pub async fn list_all(&self) -> Result<Vec<Category>, sqlx::Error> {
        query_as::<_, Category>(
            r#"
            SELECT id, slug, name, description, position, created_at
            FROM categories
            ORDER BY position ASC, id ASC
            "#,
        )
        .fetch_all(&self.db_pool)
        .await
    }

    pub async fn find_by_slug(&self, slug: &str) -> Result<Option<Category>, sqlx::Error> {
        query_as::<_, Category>(
            r#"
            SELECT id, slug, name, description, position, created_at
            FROM categories
            WHERE slug = $1
            "#,
        )
        .bind(slug)
        .fetch_optional(&self.db_pool)
        .await
    }
}
