use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::{auth::RequireAdmin, error::AppError, models, AppState};
use super::ApiResult;

pub async fn list_players(
    State(state): State<AppState>,
    RequireAdmin(_): RequireAdmin,
) -> ApiResult<Vec<models::PlayerWithToken>> {
    let rows = sqlx::query_as::<_, models::PlayerWithToken>(
        "SELECT id, name, token, created_at FROM tokens WHERE is_admin = 0 ORDER BY name",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

pub async fn create_player(
    State(state): State<AppState>,
    RequireAdmin(_): RequireAdmin,
    Json(body): Json<models::CreatePlayerRequest>,
) -> Result<(StatusCode, Json<models::PlayerWithToken>), AppError> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("Name cannot be empty".to_string()));
    }

    let token = match body.token.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => t.to_string(),
        None => uuid::Uuid::new_v4().to_string(),
    };

    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO tokens (token, name, is_admin) VALUES (?, ?, 0) RETURNING id",
    )
    .bind(&token)
    .bind(&name)
    .fetch_one(&state.db)
    .await?;

    let row = sqlx::query_as::<_, models::PlayerWithToken>(
        "SELECT id, name, token, created_at FROM tokens WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn delete_player(
    State(state): State<AppState>,
    RequireAdmin(_): RequireAdmin,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tokens WHERE id = ? AND is_admin = 0")
            .bind(id)
            .fetch_one(&state.db)
            .await?;

    if exists == 0 {
        return Err(AppError::NotFound(format!("Player {} not found", id)));
    }

    sqlx::query("DELETE FROM tournament_players WHERE token_id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;

    sqlx::query("DELETE FROM games WHERE reporter_id = ? OR opponent_id = ?")
        .bind(id)
        .bind(id)
        .execute(&state.db)
        .await?;

    sqlx::query("DELETE FROM tokens WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn create_tournament(
    State(state): State<AppState>,
    RequireAdmin(_): RequireAdmin,
    Json(body): Json<models::CreateTournamentRequest>,
) -> Result<(StatusCode, Json<models::Tournament>), AppError> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("Tournament name cannot be empty".to_string()));
    }

    let active_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tournaments WHERE status = 'active'")
            .fetch_one(&state.db)
            .await?;

    if active_count > 0 {
        return Err(AppError::BadRequest(
            "Archive the current tournament before creating a new one".to_string(),
        ));
    }

    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO tournaments (name, status) VALUES (?, 'active') RETURNING id",
    )
    .bind(&name)
    .fetch_one(&state.db)
    .await?;

    let row = sqlx::query_as::<_, models::Tournament>(
        "SELECT id, name, status, started_at, archived_at FROM tournaments WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn archive_tournament(
    State(state): State<AppState>,
    RequireAdmin(_): RequireAdmin,
) -> ApiResult<models::Tournament> {
    let row = sqlx::query_as::<_, models::Tournament>(
        "SELECT id, name, status, started_at, archived_at FROM tournaments WHERE status = 'active' LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("No active tournament".to_string()))?;

    sqlx::query(
        "UPDATE tournaments SET status = 'archived', archived_at = datetime('now') WHERE id = ?",
    )
    .bind(row.id)
    .execute(&state.db)
    .await?;

    let updated = sqlx::query_as::<_, models::Tournament>(
        "SELECT id, name, status, started_at, archived_at FROM tournaments WHERE id = ?",
    )
    .bind(row.id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(updated))
}

pub async fn add_tournament_player(
    State(state): State<AppState>,
    RequireAdmin(_): RequireAdmin,
    Path(tournament_id): Path<i64>,
    Json(body): Json<models::AddTournamentPlayerRequest>,
) -> Result<StatusCode, AppError> {
    let t_exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tournaments WHERE id = ?")
            .bind(tournament_id)
            .fetch_one(&state.db)
            .await?;

    if t_exists == 0 {
        return Err(AppError::NotFound(format!("Tournament {} not found", tournament_id)));
    }

    let p_exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tokens WHERE id = ? AND is_admin = 0")
            .bind(body.token_id)
            .fetch_one(&state.db)
            .await?;

    if p_exists == 0 {
        return Err(AppError::NotFound(format!("Player {} not found", body.token_id)));
    }

    sqlx::query(
        "INSERT OR IGNORE INTO tournament_players (tournament_id, token_id) VALUES (?, ?)",
    )
    .bind(tournament_id)
    .bind(body.token_id)
    .execute(&state.db)
    .await?;

    Ok(StatusCode::CREATED)
}

pub async fn remove_tournament_player(
    State(state): State<AppState>,
    RequireAdmin(_): RequireAdmin,
    Path((tournament_id, token_id)): Path<(i64, i64)>,
) -> Result<StatusCode, AppError> {
    sqlx::query(
        "DELETE FROM tournament_players WHERE tournament_id = ? AND token_id = ?",
    )
    .bind(tournament_id)
    .bind(token_id)
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_games(
    State(state): State<AppState>,
    RequireAdmin(_): RequireAdmin,
) -> ApiResult<Vec<models::GameResponse>> {
    let tournament = sqlx::query_as::<_, models::Tournament>(
        "SELECT id, name, status, started_at, archived_at FROM tournaments WHERE status = 'active' LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("No active tournament".to_string()))?;

    let games = super::fetch_games(
        &state.db,
        "g.tournament_id = ?",
        tournament.id,
    )
    .await?;

    Ok(Json(games))
}

pub async fn delete_game(
    State(state): State<AppState>,
    RequireAdmin(_): RequireAdmin,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM games WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Game {} not found", id)));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn admin_confirm_game(
    State(state): State<AppState>,
    RequireAdmin(_): RequireAdmin,
    Path(id): Path<i64>,
) -> ApiResult<models::GameResponse> {
    let result = sqlx::query(
        "UPDATE games SET status = 'confirmed', confirmed_at = datetime('now') WHERE id = ? AND status = 'pending'",
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "Game {} not found or not in pending state",
            id
        )));
    }

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

    Ok(Json(models::GameResponse::from(row)))
}

pub async fn admin_create_game(
    State(state): State<AppState>,
    RequireAdmin(_): RequireAdmin,
    Json(body): Json<models::AdminCreateGameRequest>,
) -> Result<(StatusCode, Json<models::GameResponse>), AppError> {
    if body.player1_id == body.player2_id {
        return Err(AppError::BadRequest("Players must be different".to_string()));
    }

    super::validate_match_score(body.player1_game_wins, body.player2_game_wins, body.game_draws)?;

    let tournament_id = match body.tournament_id {
        Some(id) => id,
        None => {
            sqlx::query_scalar::<_, i64>(
                "SELECT id FROM tournaments WHERE status = 'active' LIMIT 1",
            )
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::BadRequest("No active tournament".to_string()))?
        }
    };

    // Verify both players are in this tournament
    for pid in [body.player1_id, body.player2_id] {
        let in_t: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tournament_players WHERE tournament_id = ? AND token_id = ?",
        )
        .bind(tournament_id)
        .bind(pid)
        .fetch_one(&state.db)
        .await?;

        if in_t == 0 {
            return Err(AppError::BadRequest(format!(
                "Player {} is not in tournament {}",
                pid, tournament_id
            )));
        }
    }

    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO games (tournament_id, reporter_id, opponent_id,
                            reporter_game_wins, opponent_game_wins, game_draws,
                            status, confirmed_at)
         VALUES (?, ?, ?, ?, ?, ?, 'confirmed', datetime('now'))
         RETURNING id",
    )
    .bind(tournament_id)
    .bind(body.player1_id)
    .bind(body.player2_id)
    .bind(body.player1_game_wins)
    .bind(body.player2_game_wins)
    .bind(body.game_draws)
    .fetch_one(&state.db)
    .await?;

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

    Ok((StatusCode::CREATED, Json(models::GameResponse::from(row))))
}
