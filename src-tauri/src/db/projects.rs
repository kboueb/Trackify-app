use sqlx::SqliteExecutor;

use super::{
    models::{Project, ProjectInput},
    new_id,
};
use crate::error::{AppError, AppResult, is_unique_violation};

macro_rules! columns {
    () => {
        "id, name, client, color, archived, last_used_at, created_at, updated_at"
    };
}

/// Nettoie et valide la saisie. Seuls les espaces en début et fin sont retirés :
/// le nom doit pouvoir correspondre exactement à celui du timesheet officiel.
fn normalize(input: ProjectInput) -> AppResult<ProjectInput> {
    let name = input.name.trim().to_string();
    let client = input.client.trim().to_string();
    let color = input.color.trim().to_lowercase();
    if name.is_empty() {
        return Err(AppError::Validation("Le nom du projet est obligatoire.".into()));
    }
    let valid_color = color.len() == 7
        && color.starts_with('#')
        && color[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !valid_color {
        return Err(AppError::Validation(format!("Couleur invalide : « {color} ».")));
    }
    Ok(ProjectInput { name, client, color })
}

fn map_unique(e: sqlx::Error, input: &ProjectInput) -> AppError {
    if is_unique_violation(&e) {
        let client = if input.client.is_empty() {
            "sans client".to_string()
        } else {
            format!("pour « {} »", input.client)
        };
        AppError::Validation(format!("Le projet « {} » existe déjà ({client}).", input.name))
    } else {
        e.into()
    }
}

pub async fn list(db: impl SqliteExecutor<'_>, include_archived: bool) -> AppResult<Vec<Project>> {
    let sql = concat!(
        "SELECT ", columns!(), " FROM projects
         WHERE deleted_at IS NULL AND (? OR archived = 0)
         ORDER BY archived, client COLLATE NOCASE, name COLLATE NOCASE"
    );
    Ok(sqlx::query_as(sql).bind(include_archived).fetch_all(db).await?)
}

/// Projets non archivés les plus récemment utilisés (menu du tray, palette).
pub async fn recent(db: impl SqliteExecutor<'_>, limit: i64) -> AppResult<Vec<Project>> {
    let sql = concat!(
        "SELECT ", columns!(), " FROM projects
         WHERE deleted_at IS NULL AND archived = 0
         ORDER BY last_used_at IS NULL, last_used_at DESC, name COLLATE NOCASE
         LIMIT ?"
    );
    Ok(sqlx::query_as(sql).bind(limit).fetch_all(db).await?)
}

pub async fn get(db: impl SqliteExecutor<'_>, id: &str) -> AppResult<Project> {
    let sql = concat!("SELECT ", columns!(), " FROM projects WHERE id = ? AND deleted_at IS NULL");
    sqlx::query_as(sql)
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Projet introuvable.".into()))
}

pub async fn create(db: impl SqliteExecutor<'_>, input: ProjectInput, now: i64) -> AppResult<Project> {
    let input = normalize(input)?;
    let sql = concat!(
        "INSERT INTO projects (id, name, client, color, archived, created_at, updated_at)
         VALUES (?, ?, ?, ?, 0, ?, ?) RETURNING ", columns!(), ""
    );
    sqlx::query_as(sql)
        .bind(new_id())
        .bind(&input.name)
        .bind(&input.client)
        .bind(&input.color)
        .bind(now)
        .bind(now)
        .fetch_one(db)
        .await
        .map_err(|e| map_unique(e, &input))
}

pub async fn update(db: impl SqliteExecutor<'_>, id: &str, input: ProjectInput, now: i64) -> AppResult<Project> {
    let input = normalize(input)?;
    let sql = concat!(
        "UPDATE projects SET name = ?, client = ?, color = ?, updated_at = ?
         WHERE id = ? AND deleted_at IS NULL RETURNING ", columns!(), ""
    );
    sqlx::query_as(sql)
        .bind(&input.name)
        .bind(&input.client)
        .bind(&input.color)
        .bind(now)
        .bind(id)
        .fetch_optional(db)
        .await
        .map_err(|e| map_unique(e, &input))?
        .ok_or_else(|| AppError::NotFound("Projet introuvable.".into()))
}

/// Renomme un client sur tous ses projets (archivés compris). Renommer vers un client
/// existant fusionne les deux. Renvoie le nombre de projets modifiés.
pub async fn rename_client(db: impl SqliteExecutor<'_>, from: &str, to: &str, now: i64) -> AppResult<u64> {
    let to = to.trim();
    if from.is_empty() {
        return Err(AppError::Validation("Choisis un client à renommer.".into()));
    }
    let res = sqlx::query("UPDATE projects SET client = ?, updated_at = ? WHERE client = ? AND deleted_at IS NULL")
        .bind(to)
        .bind(now)
        .bind(from)
        .execute(db)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                AppError::Validation(format!(
                    "« {to} » a déjà un projet du même nom qu'un projet de « {from} »."
                ))
            } else {
                e.into()
            }
        })?;
    Ok(res.rows_affected())
}

pub async fn set_archived(db: impl SqliteExecutor<'_>, id: &str, archived: bool, now: i64) -> AppResult<Project> {
    let sql = concat!(
        "UPDATE projects SET archived = ?, updated_at = ?
         WHERE id = ? AND deleted_at IS NULL RETURNING ", columns!(), ""
    );
    sqlx::query_as(sql)
        .bind(archived)
        .bind(now)
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Projet introuvable.".into()))
}

/// Suppression logique : les entrées de temps existantes restent rattachées au projet.
pub async fn soft_delete(db: impl SqliteExecutor<'_>, id: &str, now: i64) -> AppResult<()> {
    let res = sqlx::query("UPDATE projects SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL")
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("Projet introuvable.".into()));
    }
    Ok(())
}

pub async fn touch_last_used(db: impl SqliteExecutor<'_>, id: &str, now: i64) -> AppResult<()> {
    sqlx::query("UPDATE projects SET last_used_at = ? WHERE id = ?")
        .bind(now)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}
