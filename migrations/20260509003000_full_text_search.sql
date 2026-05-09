ALTER TABLE threads
ADD COLUMN title_tsv tsvector;

ALTER TABLE posts
ADD COLUMN body_tsv tsvector;

UPDATE threads
SET title_tsv = to_tsvector('pg_catalog.english', title);

UPDATE posts
SET body_tsv = to_tsvector('pg_catalog.english', body_md);

ALTER TABLE threads
ALTER COLUMN title_tsv SET NOT NULL;

ALTER TABLE posts
ALTER COLUMN body_tsv SET NOT NULL;

CREATE INDEX idx_threads_title_tsv ON threads USING GIN (title_tsv);
CREATE INDEX idx_posts_body_tsv ON posts USING GIN (body_tsv);

CREATE TRIGGER threads_title_tsv_update
BEFORE INSERT OR UPDATE OF title ON threads
FOR EACH ROW
EXECUTE FUNCTION tsvector_update_trigger(title_tsv, 'pg_catalog.english', title);

CREATE TRIGGER posts_body_tsv_update
BEFORE INSERT OR UPDATE OF body_md ON posts
FOR EACH ROW
EXECUTE FUNCTION tsvector_update_trigger(body_tsv, 'pg_catalog.english', body_md);
