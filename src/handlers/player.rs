use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};

use crate::{auth::RequireToken, error::AppError, models, AppState};
use super::{ApiResult, validate_match_score};

pub async fn me(
    State(state): State<AppState>,
    RequireToken(token): RequireToken,
) -> ApiResult<Value> {
    let tournament = sqlx::query_as::<_, models::Tournament>(
        "SELECT id, name, status, started_at, archived_at FROM tournaments WHERE status = 'active' LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await?;

    let standing = if let Some(ref t) = tournament {
        let standings = super::compute_standings(&state.db, t.id).await?;
        standings.into_iter().find(|s| s.player_id == token.id)
    } else {
        None
    };

    Ok(Json(json!({
        "id": token.id,
        "name": token.name,
        "is_admin": token.is_admin(),
        "tournament": tournament,
        "standing": standing,
    })))
}

pub async fn report_game(
    State(state): State<AppState>,
    RequireToken(token): RequireToken,
    Json(body): Json<models::ReportGameRequest>,
) -> Result<(StatusCode, Json<models::GameResponse>), AppError> {
    if body.opponent_id == token.id {
        return Err(AppError::BadRequest("Cannot report a game against yourself".to_string()));
    }

    validate_match_score(body.reporter_game_wins, body.opponent_game_wins, body.game_draws)?;

    let tournament = sqlx::query_as::<_, models::Tournament>(
        "SELECT id, name, status, started_at, archived_at FROM tournaments WHERE status = 'active' LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("No active tournament".to_string()))?;

    // Both players must be in the active tournament
    let reporter_in: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tournament_players WHERE tournament_id = ? AND token_id = ?",
    )
    .bind(tournament.id)
    .bind(token.id)
    .fetch_one(&state.db)
    .await?;

    if reporter_in == 0 {
        return Err(AppError::Forbidden);
    }

    let opponent_in: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tournament_players WHERE tournament_id = ? AND token_id = ?",
    )
    .bind(tournament.id)
    .bind(body.opponent_id)
    .fetch_one(&state.db)
    .await?;

    if opponent_in == 0 {
        return Err(AppError::BadRequest("Opponent is not in the active tournament".to_string()));
    }

    // No duplicate pending game between these two players
    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM games
         WHERE tournament_id = ? AND status = 'pending'
         AND ((reporter_id = ? AND opponent_id = ?) OR (reporter_id = ? AND opponent_id = ?))",
    )
    .bind(tournament.id)
    .bind(token.id)
    .bind(body.opponent_id)
    .bind(body.opponent_id)
    .bind(token.id)
    .fetch_one(&state.db)
    .await?;

    if pending > 0 {
        return Err(AppError::BadRequest(
            "A pending game between these players already exists".to_string(),
        ));
    }

    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO games (tournament_id, reporter_id, opponent_id, reporter_game_wins, opponent_game_wins, game_draws)
         VALUES (?, ?, ?, ?, ?, ?)
         RETURNING id",
    )
    .bind(tournament.id)
    .bind(token.id)
    .bind(body.opponent_id)
    .bind(body.reporter_game_wins)
    .bind(body.opponent_game_wins)
    .bind(body.game_draws)
    .fetch_one(&state.db)
    .await?;

    let game = fetch_game(&state, id).await?;
    Ok((StatusCode::CREATED, Json(game)))
}

pub async fn pending_games(
    State(state): State<AppState>,
    RequireToken(token): RequireToken,
) -> ApiResult<Vec<models::GameResponse>> {
    let games = super::fetch_games(
        &state.db,
        "g.opponent_id = ? AND g.status = 'pending'",
        token.id,
    )
    .await?;
    Ok(Json(games))
}

// All games involving me in the active tournament (all statuses).
pub async fn my_games(
    State(state): State<AppState>,
    RequireToken(token): RequireToken,
) -> ApiResult<Vec<models::GameResponse>> {
    let tournament_id: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM tournaments WHERE status = 'active' LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await?;

    let Some(tid) = tournament_id else {
        return Ok(Json(vec![]));
    };

    let rows = sqlx::query_as::<_, models::GameRow>(
        "SELECT g.id, g.tournament_id,
                g.reporter_id, r.name AS reporter_name,
                g.opponent_id, o.name AS opponent_name,
                g.reporter_game_wins, g.opponent_game_wins, g.game_draws,
                g.status, g.reported_at, g.confirmed_at
         FROM games g
         JOIN tokens r ON r.id = g.reporter_id
         JOIN tokens o ON o.id = g.opponent_id
         WHERE g.tournament_id = ?
           AND (g.reporter_id = ? OR g.opponent_id = ?)
         ORDER BY g.reported_at DESC",
    )
    .bind(tid)
    .bind(token.id)
    .bind(token.id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(models::GameResponse::from).collect()))
}

pub async fn confirm_game(
    State(state): State<AppState>,
    RequireToken(token): RequireToken,
    Path(id): Path<i64>,
) -> ApiResult<models::GameResponse> {
    let game = get_pending_game_for_opponent(&state, id, token.id).await?;
    drop(game);

    sqlx::query(
        "UPDATE games SET status = 'confirmed', confirmed_at = datetime('now') WHERE id = ?",
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    Ok(Json(fetch_game(&state, id).await?))
}

pub async fn dispute_game(
    State(state): State<AppState>,
    RequireToken(token): RequireToken,
    Path(id): Path<i64>,
) -> ApiResult<models::GameResponse> {
    let game = get_pending_game_for_opponent(&state, id, token.id).await?;
    drop(game);

    sqlx::query("UPDATE games SET status = 'disputed' WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(Json(fetch_game(&state, id).await?))
}

async fn get_pending_game_for_opponent(
    state: &AppState,
    game_id: i64,
    token_id: i64,
) -> Result<models::GameRow, AppError> {
    let row = sqlx::query_as::<_, models::GameRow>(
        "SELECT g.id, g.tournament_id,
                g.reporter_id, r.name AS reporter_name,
                g.opponent_id, o.name AS opponent_name,
                g.reporter_game_wins, g.opponent_game_wins, g.game_draws,
                g.status, g.reported_at, g.confirmed_at
         FROM games g
         JOIN tokens r ON r.id = g.reporter_id
         JOIN tokens o ON o.id = g.opponent_id
         WHERE g.id = ?",
    )
    .bind(game_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Game {} not found", game_id)))?;

    if row.opponent_id != token_id {
        return Err(AppError::Forbidden);
    }
    if row.status != "pending" {
        return Err(AppError::BadRequest(format!("Game is not pending (status: {})", row.status)));
    }

    Ok(row)
}

async fn fetch_game(state: &AppState, id: i64) -> Result<models::GameResponse, AppError> {
    let row = sqlx::query_as::<_, models::GameRow>(
        "SELECT g.id, g.tournament_id,
                g.reporter_id, r.name AS reporter_name,
                g.opponent_id, o.name AS opponent_name,
                g.reporter_game_wins, g.opponent_game_wins, g.game_draws,
                g.status, g.reported_at, g.confirmed_at
         FROM games g
         JOIN tokens r ON r.id = g.reporter_id
         JOIN tokens o ON o.id = g.opponent_id
         WHERE g.id = ?",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    Ok(models::GameResponse::from(row))
}
