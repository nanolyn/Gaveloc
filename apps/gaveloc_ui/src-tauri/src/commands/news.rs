use base64::{engine::general_purpose::STANDARD, Engine};
use gaveloc_core::entities::{Banner, Headlines, NewsArticle};
use gaveloc_core::ports::NewsRepository;
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn get_headlines(state: State<'_, AppState>, language: String) -> Result<Headlines, String> {
    state
        .news_repository
        .get_headlines(&language)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_banners(state: State<'_, AppState>, language: String) -> Result<Vec<Banner>, String> {
    state
        .news_repository
        .get_banners(&language)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_news_article(state: State<'_, AppState>, url: String) -> Result<NewsArticle, String> {
    state
        .news_repository
        .get_article(&url)
        .await
        .map_err(|e| e.to_string())
}

/// Proxy an image URL through the backend to avoid CORS/hotlinking issues
/// Returns a base64 data URL
#[tauri::command]
pub async fn proxy_image(url: String) -> Result<String, String> {
    let client = reqwest::Client::new();

    let response = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();

    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    let base64 = STANDARD.encode(&bytes);

    Ok(format!("data:{};base64,{}", content_type, base64))
}
