use serde::{Deserialize, Serialize};

// ── DB row types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TokenRow {
    pub id: i64,
    pub token: String,
    pub name: String,
    pub is_admin: i64,
    pub created_at: String,
}

impl TokenRow {
    pub fn is_admin(&self) -> bool {
        self.is_admin != 0
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct GameRow {
    pub id: i64,
    pub tournament_id: i64,
    pub reporter_id: i64,
    pub reporter_name: String,
    pub opponent_id: i64,
    pub opponent_name: String,
    pub reporter_game_wins: i64,
    pub opponent_game_wins: i64,
    pub game_draws: i64,
    pub status: String,
    pub reported_at: String,
    pub confirmed_at: Option<String>,
}

// ── API response types ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Tournament {
    pub id: i64,
    pub name: String,
    pub status: String,
    pub started_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow, Clone)]
pub struct PlayerInfo {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PlayerWithToken {
    pub id: i64,
    pub name: String,
    pub token: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct GameResponse {
    pub id: i64,
    pub tournament_id: i64,
    pub reporter: PlayerInfo,
    pub opponent: PlayerInfo,
    pub reporter_game_wins: i64,
    pub opponent_game_wins: i64,
    pub game_draws: i64,
    pub match_result: String,
    pub status: String,
    pub reported_at: String,
    pub confirmed_at: Option<String>,
}

impl From<GameRow> for GameResponse {
    fn from(r: GameRow) -> Self {
        let match_result = if r.reporter_game_wins > r.opponent_game_wins {
            "reporter_win"
        } else if r.reporter_game_wins < r.opponent_game_wins {
            "opponent_win"
        } else {
            "draw"
        }
        .to_string();

        GameResponse {
            id: r.id,
            tournament_id: r.tournament_id,
            reporter: PlayerInfo { id: r.reporter_id, name: r.reporter_name },
            opponent: PlayerInfo { id: r.opponent_id, name: r.opponent_name },
            reporter_game_wins: r.reporter_game_wins,
            opponent_game_wins: r.opponent_game_wins,
            game_draws: r.game_draws,
            match_result,
            status: r.status,
            reported_at: r.reported_at,
            confirmed_at: r.confirmed_at,
        }
    }
}

// Standings: computed in-memory from confirmed games.
//
// Ranking method: Match Points Rate (MPR) = (3W + D) / (3N)
// Tiebreaker: Game Win Rate (GWR) = game_wins / total_games_played
//
// Alternatives considered:
//   - Raw match points (3W+D): biased toward players with more games
//   - Win rate (W/N): ignores draws
//   - ELO: handles strength-of-schedule but requires many games to converge
//
// MPR mirrors WotC Organized Play tiebreaker logic and normalizes fairly
// for unequal game counts.
#[derive(Debug, Serialize)]
pub struct Standing {
    pub rank: usize,
    pub player_id: i64,
    pub player_name: String,
    pub match_wins: i64,
    pub match_draws: i64,
    pub match_losses: i64,
    pub total_matches: i64,
    pub match_points: i64,
    pub match_points_rate: f64,
    pub game_wins: i64,
    pub game_losses: i64,
    pub game_draws_count: i64,
    pub total_games: i64,
    pub game_win_rate: f64,
}

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ReportGameRequest {
    pub opponent_id: i64,
    pub reporter_game_wins: i64,
    pub opponent_game_wins: i64,
    pub game_draws: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreatePlayerRequest {
    pub name: String,
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTournamentRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct AddTournamentPlayerRequest {
    pub token_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct AdminCreateGameRequest {
    pub tournament_id: Option<i64>,
    pub player1_id: i64,
    pub player2_id: i64,
    pub player1_game_wins: i64,
    pub player2_game_wins: i64,
    pub game_draws: i64,
}
