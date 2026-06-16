use axum::{
    routing::{delete, get, post},
    Router,
};
use sqlx::SqlitePool;
use tower_http::{cors::CorsLayer, services::{ServeDir, ServeFile}};

mod auth;
mod error;
mod handlers;
mod models;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("league_scoring=info".parse().unwrap()),
        )
        .init();

    let db_path = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:league.db".to_string());

    let opts = db_path
        .parse::<sqlx::sqlite::SqliteConnectOptions>()?
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(opts)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    seed_admin(&pool).await?;

    let state = AppState { db: pool };

    let app = Router::new()
        // Public
        .route("/api/tournaments", get(handlers::public::list_tournaments))
        .route("/api/tournaments/active", get(handlers::public::active_tournament))
        .route("/api/tournaments/:id/standings", get(handlers::public::tournament_standings))
        .route("/api/tournaments/:id/games", get(handlers::public::tournament_games))
        .route("/api/players", get(handlers::public::list_players))
        // Player
        .route("/api/me", get(handlers::player::me))
        .route("/api/games", post(handlers::player::report_game))
        .route("/api/games/pending", get(handlers::player::pending_games))
        .route("/api/games/mine", get(handlers::player::my_games))
        .route("/api/games/:id/confirm", post(handlers::player::confirm_game))
        .route("/api/games/:id/dispute", post(handlers::player::dispute_game))
        // Admin
        .route(
            "/api/admin/players",
            get(handlers::admin::list_players).post(handlers::admin::create_player),
        )
        .route("/api/admin/players/:id", delete(handlers::admin::delete_player))
        .route("/api/admin/tournaments", post(handlers::admin::create_tournament))
        .route("/api/admin/tournaments/archive", post(handlers::admin::archive_tournament))
        .route(
            "/api/admin/tournaments/:id/players",
            post(handlers::admin::add_tournament_player),
        )
        .route(
            "/api/admin/tournaments/:id/players/:token_id",
            delete(handlers::admin::remove_tournament_player),
        )
        .route(
            "/api/admin/games",
            get(handlers::admin::list_games).post(handlers::admin::admin_create_game),
        )
        .route("/api/admin/games/:id", delete(handlers::admin::delete_game))
        .route("/api/admin/games/:id/confirm", post(handlers::admin::admin_confirm_game))
        // Clean URLs
        .route_service("/admin", ServeFile::new("static/admin.html"))
        .route_service("/player", ServeFile::new("static/player.html"))
        // Static files (fallback)
        .fallback_service(ServeDir::new("static"))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000u16);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn seed_admin(pool: &SqlitePool) -> anyhow::Result<()> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tokens WHERE is_admin = 1")
        .fetch_one(pool)
        .await?;

    if count == 0 {
        let token = std::env::var("ADMIN_TOKEN")
            .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());

        sqlx::query("INSERT INTO tokens (token, name, is_admin) VALUES (?, 'Admin', 1)")
            .bind(&token)
            .execute(pool)
            .await?;

        tracing::info!("┌─────────────────────────────────────────────┐");
        tracing::info!("│  ADMIN TOKEN: {:<31}│", token);
        tracing::info!("└─────────────────────────────────────────────┘");
    }

    Ok(())
}
