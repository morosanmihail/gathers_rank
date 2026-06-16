# League Scoring

Self-hosted league/tournament tracker. Players report match results; opponents confirm them. Admins manage tournaments and players via a token-based API.

## Stack

- **Rust** (Axum + SQLx + SQLite) — single binary, no external DB required
- **Static HTML** front-ends for admin and player views
- **Docker** for deployment

## How it works

- Each player gets a unique token (bearer auth)
- Players report game results; the opponent confirms or disputes
- Admins create tournaments, add/remove players, and can confirm/delete any game
- Standings and games are publicly readable (no token required)

## Getting started

### Local (cargo)

```bash
cargo run
```

On first boot, an admin token is printed to stdout. Set `ADMIN_TOKEN` env var before first boot to use a specific value.

The server listens on `http://localhost:3000` by default.

### Docker Compose

```bash
# Optional: set a fixed admin token before first boot
# Edit docker-compose.yml and uncomment ADMIN_TOKEN

docker compose up -d
```

Data persists in the `league_db` Docker volume.

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | `sqlite:league.db` | SQLite database path |
| `PORT` | `3000` | HTTP listen port |
| `ADMIN_TOKEN` | *(random UUID)* | Admin token seeded on first boot |
| `RUST_LOG` | — | Log filter (e.g. `info`) |

## Pages

| URL | Description |
|---|---|
| `/` | Public standings |
| `/admin` | Admin panel |
| `/player` | Player panel |

## API overview

All player/admin routes require `Authorization: Bearer <token>`.

| Method | Path | Auth | Description |
|---|---|---|---|
| GET | `/api/tournaments` | — | List tournaments |
| GET | `/api/tournaments/active` | — | Active tournament |
| GET | `/api/tournaments/:id/standings` | — | Standings |
| GET | `/api/tournaments/:id/games` | — | Games |
| GET | `/api/players` | — | List players |
| GET | `/api/me` | Player | Own profile |
| POST | `/api/games` | Player | Report game result |
| GET | `/api/games/pending` | Player | Games awaiting confirmation |
| GET | `/api/games/mine` | Player | Own games |
| POST | `/api/games/:id/confirm` | Player | Confirm opponent's report |
| POST | `/api/games/:id/dispute` | Player | Dispute opponent's report |
| GET/POST | `/api/admin/players` | Admin | List / create players |
| DELETE | `/api/admin/players/:id` | Admin | Remove player |
| POST | `/api/admin/tournaments` | Admin | Create tournament |
| POST | `/api/admin/tournaments/archive` | Admin | Archive active tournament |
| POST | `/api/admin/tournaments/:id/players` | Admin | Add player to tournament |
| DELETE | `/api/admin/tournaments/:id/players/:token_id` | Admin | Remove player from tournament |
| GET/POST | `/api/admin/games` | Admin | List / create game |
| DELETE | `/api/admin/games/:id` | Admin | Delete game |
| POST | `/api/admin/games/:id/confirm` | Admin | Force-confirm game |
