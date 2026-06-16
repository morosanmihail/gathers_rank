pub mod admin;
pub mod player;
pub mod public;

use axum::Json;
use sqlx::SqlitePool;

use crate::{error::AppError, models};

pub type ApiResult<T> = Result<Json<T>, AppError>;

const GAME_SELECT: &str = "
    SELECT
        g.id, g.tournament_id,
        g.reporter_id, r.name AS reporter_name,
        g.opponent_id, o.name AS opponent_name,
        g.reporter_game_wins, g.opponent_game_wins, g.game_draws,
        g.status, g.reported_at, g.confirmed_at
    FROM games g
    JOIN tokens r ON r.id = g.reporter_id
    JOIN tokens o ON o.id = g.opponent_id
";

pub async fn fetch_games(
    pool: &SqlitePool,
    where_clause: &str,
    bind_val: i64,
) -> Result<Vec<models::GameResponse>, AppError> {
    let sql = format!("{} WHERE {} ORDER BY g.reported_at DESC", GAME_SELECT, where_clause);
    let rows = sqlx::query_as::<_, models::GameRow>(&sql)
        .bind(bind_val)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(models::GameResponse::from).collect())
}

pub async fn compute_standings(
    pool: &SqlitePool,
    tournament_id: i64,
) -> Result<Vec<models::Standing>, AppError> {
    let players: Vec<models::PlayerInfo> = sqlx::query_as::<_, models::PlayerInfo>(
        "SELECT t.id, t.name FROM tokens t
         JOIN tournament_players tp ON tp.token_id = t.id
         WHERE tp.tournament_id = ? AND t.is_admin = 0",
    )
    .bind(tournament_id)
    .fetch_all(pool)
    .await?;

    let games = sqlx::query_as::<_, models::GameRow>(&format!(
        "{} WHERE g.tournament_id = ? AND g.status = 'confirmed'",
        GAME_SELECT
    ))
    .bind(tournament_id)
    .fetch_all(pool)
    .await?;

    let mut standings: Vec<models::Standing> = players
        .iter()
        .map(|p| {
            let mut mw = 0i64;
            let mut md = 0i64;
            let mut ml = 0i64;
            let mut gw = 0i64;
            let mut gl = 0i64;
            let mut gd = 0i64;

            for g in &games {
                if g.reporter_id != p.id && g.opponent_id != p.id {
                    continue;
                }
                let (my_gw, opp_gw) = if g.reporter_id == p.id {
                    (g.reporter_game_wins, g.opponent_game_wins)
                } else {
                    (g.opponent_game_wins, g.reporter_game_wins)
                };
                if my_gw > opp_gw { mw += 1; }
                else if my_gw < opp_gw { ml += 1; }
                else { md += 1; }
                gw += my_gw;
                gl += opp_gw;
                gd += g.game_draws;
            }

            let total_matches = mw + md + ml;
            let match_points = 3 * mw + md;
            let match_points_rate = if total_matches > 0 {
                match_points as f64 / (3 * total_matches) as f64
            } else {
                0.0
            };
            let total_games = gw + gl + gd;
            let game_win_rate = if total_games > 0 { gw as f64 / total_games as f64 } else { 0.0 };

            models::Standing {
                rank: 0,
                player_id: p.id,
                player_name: p.name.clone(),
                match_wins: mw,
                match_draws: md,
                match_losses: ml,
                total_matches,
                match_points,
                match_points_rate,
                game_wins: gw,
                game_losses: gl,
                game_draws_count: gd,
                total_games,
                game_win_rate,
            }
        })
        .collect();

    standings.sort_by(|a, b| {
        b.match_points_rate
            .partial_cmp(&a.match_points_rate)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                b.game_win_rate
                    .partial_cmp(&a.game_win_rate)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.player_name.cmp(&b.player_name))
    });

    for (i, s) in standings.iter_mut().enumerate() {
        s.rank = i + 1;
    }

    Ok(standings)
}

pub fn validate_match_score(
    reporter_wins: i64,
    opponent_wins: i64,
    draws: i64,
) -> Result<(), AppError> {
    let total = reporter_wins + opponent_wins + draws;
    let valid = reporter_wins >= 0
        && opponent_wins >= 0
        && draws >= 0
        && total >= 1
        && total <= 3
        && reporter_wins <= 2
        && opponent_wins <= 2
        && (reporter_wins == 2 || opponent_wins == 2 || total == 3);

    if !valid {
        return Err(AppError::BadRequest(
            "Invalid match score. Valid results: 2-0, 2-1, 0-2, 1-2, or played-out draw (3 total games).".to_string(),
        ));
    }
    Ok(())
}
