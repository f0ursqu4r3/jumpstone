# SQLx Migrations

- Place migration files in this directory using SQLx naming convention (e.g. `0002_description.sql`).
- Apply migrations locally with `sqlx migrate run --source backend/migrations` once a database URL is configured.
- Commit each forward-only migration; avoid editing files after they land to preserve history.
