//! Metadata Operations Commands
//!
//! Commands for tags, favorites, color labels, ratings, comments

use tauri::State;
use std::path::Path;
use tracing::info;
use super::state::VfsStateWrapper;
use super::helpers;
use crate::vfs::domain::{ColorLabel, FileTag};
use crate::vfs::ports::metadata::IMetadataStore;

/// Get metadata for a file
#[tauri::command]
pub async fn vfs_get_metadata(
    source_id: String,
    path: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<serde_json::Value, String> {
    let store = helpers::get_metadata_store_instance().await
        .map_err(|e| format!("Failed to get metadata store: {}", e))?;
    
    let vfs_path = Path::new(&path);
    let metadata = IMetadataStore::get(&*store, &source_id, vfs_path).await
        .map_err(|e| format!("Failed to get metadata: {}", e))?;
    
    // Convert to JSON value
    let json_value = if let Some(meta) = metadata {
        serde_json::to_value(meta)
            .map_err(|e| format!("Failed to serialize metadata: {}", e))?
    } else {
        serde_json::json!({
            "tags": [],
            "is_favorite": false,
            "color_label": null,
            "rating": null,
            "comment": null
        })
    };
    
    Ok(json_value)
}

/// Add a tag to a file
#[tauri::command]
pub async fn vfs_add_tag(
    source_id: String,
    path: String,
    tag_name: String,
    tag_color: Option<String>,
    _state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    let store = helpers::get_metadata_store_instance().await
        .map_err(|e| format!("Failed to get metadata store: {}", e))?;
    
    let vfs_path = Path::new(&path);
    
    // Create FileTag with optional color
    let file_tag = if let Some(color) = tag_color.clone() {
        FileTag::with_color(tag_name.clone(), color)
    } else {
        FileTag::new(tag_name.clone())
    };
    
    let tag_name_for_log = file_tag.name.clone();
    let tag_color_for_log = file_tag.color.clone();
    
    IMetadataStore::add_tag(&*store, &source_id, vfs_path, file_tag).await
        .map_err(|e| format!("Failed to add tag: {}", e))?;
    
    info!("Added tag {} with color {:?} to {}:{}", tag_name_for_log, tag_color_for_log, source_id, path);
    Ok(())
}

/// Remove a tag from a file
#[tauri::command]
pub async fn vfs_remove_tag(
    source_id: String,
    path: String,
    tag: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    let store = helpers::get_metadata_store_instance().await
        .map_err(|e| format!("Failed to get metadata store: {}", e))?;
    
    let vfs_path = Path::new(&path);
    
    IMetadataStore::remove_tag(&*store, &source_id, vfs_path, &tag).await
        .map_err(|e| format!("Failed to remove tag: {}", e))?;
    
    info!("Removed tag from {}:{}", source_id, path);
    Ok(())
}

/// Toggle favorite status for a file
#[tauri::command]
pub async fn vfs_toggle_favorite(
    source_id: String,
    path: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<bool, String> {
    let store = helpers::get_metadata_store_instance().await
        .map_err(|e| format!("Failed to get metadata store: {}", e))?;
    
    let vfs_path = Path::new(&path);
    let new_state = IMetadataStore::toggle_favorite(&*store, &source_id, vfs_path).await
        .map_err(|e| format!("Failed to toggle favorite: {}", e))?;
    
    info!("Toggled favorite for {}:{} to {}", source_id, path, new_state);
    Ok(new_state)
}

/// Set favorite status for a file
#[tauri::command]
pub async fn vfs_set_favorite(
    source_id: String,
    path: String,
    is_favorite: bool,
    _state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    let store = helpers::get_metadata_store_instance().await
        .map_err(|e| format!("Failed to get metadata store: {}", e))?;
    
    let vfs_path = Path::new(&path);
    
    IMetadataStore::set_favorite(&*store, &source_id, vfs_path, is_favorite).await
        .map_err(|e| format!("Failed to set favorite: {}", e))?;
    
    info!("Set favorite for {}:{} to {}", source_id, path, is_favorite);
    Ok(())
}

/// Set color label for a file
#[tauri::command]
pub async fn vfs_set_color_label(
    source_id: String,
    path: String,
    label: Option<String>,
    _state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    let store = helpers::get_metadata_store_instance().await
        .map_err(|e| format!("Failed to get metadata store: {}", e))?;
    
    let vfs_path = Path::new(&path);
    
    // Convert string label to ColorLabel enum
    let color_label = if let Some(label_str) = label {
        ColorLabel::from_str(&label_str)
    } else {
        None
    };
    
    IMetadataStore::set_color_label(&*store, &source_id, vfs_path, color_label).await
        .map_err(|e| format!("Failed to set color label: {}", e))?;
    
    info!("Set color label for {}:{} to {:?}", source_id, path, color_label);
    Ok(())
}

/// Set rating for a file (0-5)
#[tauri::command]
pub async fn vfs_set_rating(
    source_id: String,
    path: String,
    rating: Option<u8>,
    _state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    let store = helpers::get_metadata_store_instance().await
        .map_err(|e| format!("Failed to get metadata store: {}", e))?;
    
    let vfs_path = Path::new(&path);
    
    // Validate rating (0-5)
    let validated_rating = rating.map(|r| r.min(5));
    
    IMetadataStore::set_rating(&*store, &source_id, vfs_path, validated_rating).await
        .map_err(|e| format!("Failed to set rating: {}", e))?;
    
    info!("Set rating for {}:{} to {:?}", source_id, path, validated_rating);
    Ok(())
}

/// Set comment for a file
#[tauri::command]
pub async fn vfs_set_comment(
    source_id: String,
    path: String,
    comment: Option<String>,
    _state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    let store = helpers::get_metadata_store_instance().await
        .map_err(|e| format!("Failed to get metadata store: {}", e))?;
    
    let vfs_path = Path::new(&path);
    
    IMetadataStore::set_comment(&*store, &source_id, vfs_path, comment.clone()).await
        .map_err(|e| format!("Failed to set comment: {}", e))?;
    
    info!("Set comment for {}:{}", source_id, path);
    Ok(())
}

/// List all favorite files for a source
#[tauri::command]
pub async fn vfs_list_favorites(
    source_id: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<Vec<serde_json::Value>, String> {
    let store = helpers::get_metadata_store_instance().await
        .map_err(|e| format!("Failed to get metadata store: {}", e))?;
    
    let paths = IMetadataStore::list_favorites(&*store, &source_id).await
        .map_err(|e| format!("Failed to list favorites: {}", e))?;
    
    // Convert paths to JSON objects with path field
    let favorites: Vec<serde_json::Value> = paths.into_iter()
        .map(|p| serde_json::json!({
            "source_id": source_id,
            "path": p
        }))
        .collect();
    
    Ok(favorites)
}

/// List all files with a specific tag
#[tauri::command]
pub async fn vfs_list_by_tag(
    source_id: String,
    tag: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<Vec<serde_json::Value>, String> {
    let store = helpers::get_metadata_store_instance().await
        .map_err(|e| format!("Failed to get metadata store: {}", e))?;
    
    let paths = IMetadataStore::list_by_tag(&*store, &source_id, &tag).await
        .map_err(|e| format!("Failed to list files by tag: {}", e))?;
    
    // Convert paths to JSON objects
    let files: Vec<serde_json::Value> = paths.into_iter()
        .map(|p| serde_json::json!({
            "source_id": source_id,
            "path": p,
            "tag": tag
        }))
        .collect();
    
    Ok(files)
}

/// List all files with a specific color label
#[tauri::command]
pub async fn vfs_list_by_color(
    source_id: String,
    color: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<Vec<serde_json::Value>, String> {
    let store = helpers::get_metadata_store_instance().await
        .map_err(|e| format!("Failed to get metadata store: {}", e))?;
    
    let color_label = ColorLabel::from_str(&color)
        .ok_or_else(|| format!("Invalid color label: {}", color))?;
    
    let paths = IMetadataStore::list_by_color(&*store, &source_id, color_label).await
        .map_err(|e| format!("Failed to list files by color: {}", e))?;
    
    // Convert paths to JSON objects
    let files: Vec<serde_json::Value> = paths.into_iter()
        .map(|p| serde_json::json!({
            "source_id": source_id,
            "path": p,
            "color": color
        }))
        .collect();
    
    Ok(files)
}

/// List all unique tags used in a source
#[tauri::command]
pub async fn vfs_list_all_tags(
    source_id: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<Vec<serde_json::Value>, String> {
    let store = helpers::get_metadata_store_instance().await
        .map_err(|e| format!("Failed to get metadata store: {}", e))?;
    
    let tags = IMetadataStore::list_all_tags(&*store, &source_id).await
        .map_err(|e| format!("Failed to list tags: {}", e))?;
    
    // Convert FileTag to JSON objects with name and optional color
    let tag_list: Vec<serde_json::Value> = tags.into_iter()
        .map(|tag| {
            let mut obj = serde_json::json!({
                "name": tag.name
            });
            if let Some(color) = tag.color {
                obj["color"] = serde_json::Value::String(color);
            }
            obj
        })
        .collect();
    
    Ok(tag_list)
}
