CREATE TABLE IF NOT EXISTS users
(
    id INTEGER PRIMARY KEY NOT NULL,
    user_id VARCHAR(36) NOT NULL,
    email TEXT NOT NULL,
    display_name TEXT,
    password_hash TEXT NOT NULL,
    avatar_url TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
)
