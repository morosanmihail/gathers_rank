use async_trait::async_trait;
use axum::{extract::FromRequestParts, http::request::Parts};
use crate::{error::AppError, models::TokenRow, AppState};

pub struct RequireToken(pub TokenRow);
pub struct RequireAdmin(pub TokenRow);

fn extract_bearer(parts: &Parts) -> Option<String> {
    parts
        .headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string())
}

#[async_trait]
impl FromRequestParts<AppState> for RequireToken {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = extract_bearer(parts).ok_or(AppError::Unauthorized)?;

        let row = sqlx::query_as::<_, TokenRow>(
            "SELECT id, token, name, is_admin, created_at FROM tokens WHERE token = ?",
        )
        .bind(&token)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::from)?
        .ok_or(AppError::Unauthorized)?;

        Ok(RequireToken(row))
    }
}

#[async_trait]
impl FromRequestParts<AppState> for RequireAdmin {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let RequireToken(token) = RequireToken::from_request_parts(parts, state).await?;

        if !token.is_admin() {
            return Err(AppError::Forbidden);
        }

        Ok(RequireAdmin(token))
    }
}
