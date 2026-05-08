CREATE TABLE threads (
    id BIGSERIAL PRIMARY KEY,
    category_id BIGINT NOT NULL REFERENCES categories (id) ON DELETE RESTRICT,
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE RESTRICT,
    title TEXT NOT NULL,
    slug TEXT NOT NULL,
    is_pinned BOOLEAN NOT NULL DEFAULT FALSE,
    is_locked BOOLEAN NOT NULL DEFAULT FALSE,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_activity_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT threads_category_slug_key UNIQUE (category_id, slug)
);

CREATE INDEX idx_threads_category_id ON threads (category_id);
CREATE INDEX idx_threads_user_id ON threads (user_id);
CREATE INDEX idx_threads_last_activity_at ON threads (last_activity_at DESC);

CREATE TABLE posts (
    id BIGSERIAL PRIMARY KEY,
    thread_id BIGINT NOT NULL REFERENCES threads (id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE RESTRICT,
    body_md TEXT NOT NULL,
    body_html TEXT NOT NULL,
    edited_at TIMESTAMPTZ,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_posts_thread_id ON posts (thread_id);
CREATE INDEX idx_posts_user_id ON posts (user_id);
