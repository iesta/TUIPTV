use crate::config::Config;
use crate::db::Database;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct XtreamCategory {
    pub category_id: String,
    pub category_name: String,
}

#[derive(Debug, Deserialize)]
pub struct XtreamMovie {
    pub name: String,
    #[serde(deserialize_with = "parse_category_id")]
    pub category_id: String,
    pub stream_id: Option<i64>,
    pub container_extension: Option<String>,
    pub stream_icon: Option<String>,
    pub rating: Option<String>,
    pub release_date: Option<String>,
    pub plot: Option<String>,
    pub num: Option<i64>,
    #[serde(default)]
    pub genre: Option<String>,
}

fn parse_category_id<'de, D>(d: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;
    match Value::deserialize(d)? {
        Value::String(s) => Ok(s),
        Value::Array(arr) => arr
            .first()
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| de::Error::custom("expected non-empty string array")),
        Value::Number(n) => Ok(n.to_string()),
        other => Err(de::Error::custom(format!(
            "unexpected category_id type: {other:?}"
        ))),
    }
}

pub async fn sync_vod_categories(config: &Config, db: &Database) -> Result<usize> {
    let url = format!("{}&action=get_vod_categories", config.api_base());
    let body = reqwest::get(&url).await?.text().await?;
    let categories: Vec<XtreamCategory> = serde_json::from_str(&body)
        .with_context(|| format!("expected array from get_vod_categories, got: {:.200}", body))?;

    let conn = db.conn.lock().unwrap();
    let mut stmt = conn.prepare("INSERT OR REPLACE INTO categories (id, name) VALUES (?1, ?2)")?;
    for cat in &categories {
        let id: i64 = cat.category_id.parse().unwrap_or(0);
        stmt.execute(rusqlite::params![id, cat.category_name])?;
    }
    Ok(categories.len())
}

pub async fn sync_vod_streams(config: &Config, db: &Database) -> Result<usize> {
    let url = format!("{}&action=get_vod_streams", config.api_base());
    let body = reqwest::get(&url).await?.text().await?;
    let streams: Vec<XtreamMovie> = serde_json::from_str(&body)
        .with_context(|| format!("expected array from get_vod_streams, got: {:.200}", body))?;

    let conn = db.conn.lock().unwrap();
    conn.execute_batch("BEGIN")?;
    let mut stmt = conn.prepare(
        "INSERT OR REPLACE INTO movies (id, name, category_id, stream_id, container_extension, stream_icon, rating, release_date, plot, genre)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )?;
    for m in &streams {
        let id = m.stream_id.unwrap_or(0);
        let category_id: i64 = m.category_id.parse().unwrap_or(0);
        stmt.execute(rusqlite::params![
            id,
            m.name,
            category_id,
            m.stream_id,
            m.container_extension,
            m.stream_icon,
            m.rating,
            m.release_date,
            m.plot,
            m.genre,
        ])?;
    }
    conn.execute_batch("COMMIT")?;
    Ok(streams.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_movie_with_string_category() {
        let json = r#"{"name":"Test","category_id":"5"}"#;
        let m: XtreamMovie = serde_json::from_str(json).unwrap();
        assert_eq!(m.name, "Test");
        assert_eq!(m.category_id, "5");
    }

    #[test]
    fn deserialize_movie_with_number_category() {
        let json = r#"{"name":"Test","category_id":7}"#;
        let m: XtreamMovie = serde_json::from_str(json).unwrap();
        assert_eq!(m.category_id, "7");
    }

    #[test]
    fn deserialize_movie_with_array_category() {
        let json = r#"{"name":"Test","category_id":["12"]}"#;
        let m: XtreamMovie = serde_json::from_str(json).unwrap();
        assert_eq!(m.category_id, "12");
    }

    #[test]
    fn deserialize_with_minimal_fields() {
        let json = r#"{"name":"Minimal","category_id":"1"}"#;
        let m: XtreamMovie = serde_json::from_str(json).unwrap();
        assert_eq!(m.name, "Minimal");
        assert!(m.stream_icon.is_none());
        assert!(m.rating.is_none());
        assert!(m.plot.is_none());
        assert!(m.genre.is_none());
    }

    #[test]
    fn deserialize_category() {
        let json = r#"{"category_id":"3","category_name":"Action"}"#;
        let c: XtreamCategory = serde_json::from_str(json).unwrap();
        assert_eq!(c.category_name, "Action");
    }
}
