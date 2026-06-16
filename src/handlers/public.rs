use axum::{
    extract::{Path, State},
    Json,
};

use crate::{error::AppError, models, AppState};
use super::{ApiResult, compute_standings};

pub async fn list_tournaments(
    State(state): State<AppState>,
) -> ApiResult<Vec<models::Tournament>> {
    let rows = sqlx::query_as::<_, models::Tournament>(
        "SELECT id, name, status, started_at, archived_at FROM tournaments ORDER BY started_at DESC",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

pub async fn active_tournament(
    State(state): State<AppState>,
) -> ApiResult<models::Tournament> {
    let row = sqlx::query_as::<_, models::Tournament>(
        "SELECT id, name, status, started_at, archived_at FROM tournaments WHERE status = 'active' LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("No active tournament".to_string()))?;
    Ok(Json(row))
}

pub async fn tournament_standings(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Vec<models::Standing>> {
    ensure_tournament_exists(&state, id).await?;
    let standings = compute_standings(&state.db, id).await?;
    Ok(Json(standings))
}

pub async fn tournament_games(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Vec<models::GameResponse>> {
    ensure_tournament_exists(&state, id).await?;
    let games = super::fetch_games(
        &state.db,
        "g.tournament_id = ? AND g.status = 'confirmed'",
        id,
    )
    .await?;
    Ok(Json(games))
}

pub async fn list_players(
    State(state): State<AppState>,
) -> ApiResult<Vec<models::PlayerInfo>> {
    let rows = sqlx::query_as::<_, models::PlayerInfo>(
        "SELECT id, name FROM tokens WHERE is_admin = 0 ORDER BY name",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

async fn ensure_tournament_exists(state: &AppState, id: i64) -> Result<(), AppError> {
    let exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tournaments WHERE id = ?")
            .bind(id)
            .fetch_one(&state.db)
            .await?;
    if exists == 0 {
        return Err(AppError::NotFound(format!("Tournament {} not found", id)));
    }
    Ok(())
}
