CREATE TABLE IF NOT EXISTS shortened_urls (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    short_code      VARCHAR(7) NOT NULL UNIQUE,
    original_url    TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ NOT NULL,
    click_count     BIGINT NOT NULL DEFAULT 0,
    last_clicked_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_expires_at ON shortened_urls (expires_at);
