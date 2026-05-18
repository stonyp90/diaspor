//! Diaspor VFS - Virtual Cloud File System
//!
//! A multi-tier cloud storage file browser with DAM/MAM features.

pub mod vfs;
pub mod gpu;
pub mod system;
pub mod commands;
pub mod logging;
pub mod settings;

use tauri::{Manager, tray::TrayIconEvent};
use vfs::commands::VfsStateWrapper;
use vfs::infrastructure::MediaStateWrapper;

// ============================================================================
// Window Commands
// ============================================================================

#[tauri::command]
fn show_window(window: tauri::Window) {
    if let Some(webview_window) = window.get_webview_window("main") {
        let _ = webview_window.show();
        let _ = webview_window.set_focus();
    }
}

#[tauri::command]
fn hide_window(window: tauri::Window) {
    if let Some(webview_window) = window.get_webview_window("main") {
        let _ = webview_window.hide();
    }
}

#[tauri::command]
fn toggle_window(window: tauri::Window) {
    if let Some(webview_window) = window.get_webview_window("main") {
        if webview_window.is_visible().unwrap_or(false) {
            let _ = webview_window.hide();
        } else {
            let _ = webview_window.show();
            let _ = webview_window.set_focus();
        }
    }
}

// ============================================================================
// Developer Tools Toggle
// ============================================================================

#[tauri::command]
fn toggle_devtools(_window: tauri::Window) {
    #[cfg(debug_assertions)]
    if let Some(webview_window) = _window.get_webview_window("main") {
        let _ = webview_window.eval("console.log('DevTools toggled')");
    }
}

#[tauri::command]
fn open_devtools(window: tauri::Window) -> Result<(), String> {
    #[cfg(debug_assertions)]
    if let Some(webview_window) = window.get_webview_window("main") {
        let platform = std::env::consts::OS;
        
        // Try to open devtools using JavaScript
        // In Tauri v2, devtools are enabled in config, so we can try to trigger them
        let shortcut = if platform == "macos" {
            "Cmd+Option+I"
        } else {
            "Ctrl+Shift+I"
        };
        
        // Use JavaScript to try opening devtools
        // This works if devtools are enabled in tauri.conf.json
        let (ctrl_key, meta_key, alt_key) = if platform == "macos" {
            ("false", "true", "true")
        } else {
            ("true", "false", "false")
        };
        
        let js_code = format!(
            r#"
            (function() {{
                console.log('Attempting to open DevTools...');
                console.log('If DevTools do not open automatically, use: Right-click -> Inspect Element, or {}');
                
                // Try to trigger devtools via keyboard event simulation
                try {{
                    // Create and dispatch keyboard event
                    const event = new KeyboardEvent('keydown', {{
                        key: 'i',
                        code: 'KeyI',
                        keyCode: 73,
                        which: 73,
                        ctrlKey: {},
                        shiftKey: true,
                        metaKey: {},
                        altKey: {},
                        bubbles: true,
                        cancelable: true
                    }});
                    document.dispatchEvent(event);
                }} catch (e) {{
                    console.log('Keyboard event simulation failed:', e);
                }}
            }})();
            "#,
            shortcut, ctrl_key, meta_key, alt_key
        );
        
        if let Err(e) = webview_window.eval(&js_code) {
            tracing::warn!("Failed to execute devtools JavaScript: {}", e);
        }
        
        tracing::info!("DevTools access attempted for {} - use Right-click -> Inspect Element or {}", platform, shortcut);
        Ok(())
    } else {
        Err("Main window not found".to_string())
    }

    #[cfg(not(debug_assertions))]
    {
        let _ = window;
        Err("DevTools are disabled in production for security reasons".to_string())
    }
}

#[tauri::command]
fn close_devtools(_window: tauri::Window) {
    #[cfg(debug_assertions)]
    tracing::info!("DevTools closed");
}

// ============================================================================
// Application Entry Point
// ============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize settings first (before logging, as logging path may come from settings)
    let settings = settings::get_settings();
    
    // Initialize file-based logging using settings
    let log_dir = settings.get_log_directory();
    
    // Only initialize file logging if enabled in settings
    let logging_settings = settings.get_logging();
    if logging_settings.enable_file_logging.unwrap_or(true) {
        let max_file_size = logging_settings.max_file_size.unwrap_or(10 * 1024 * 1024);
        let max_rotated_files = logging_settings.max_rotated_files.unwrap_or(5);
        
        match logging::init_file_logging_with_settings(&log_dir, max_file_size, max_rotated_files) {
            Ok(_) => {
                tracing::info!(
                    "File logging initialized at: {:?} (max_size: {}MB, max_files: {}, platform: {})", 
                    log_dir, 
                    max_file_size / (1024 * 1024), 
                    max_rotated_files,
                    std::env::consts::OS
                );
            }
            Err(e) => {
                eprintln!("⚠️ Failed to initialize file logging: {}", e);
                eprintln!("⚠️ Log directory: {:?}", log_dir);
                eprintln!("⚠️ Platform: {}", std::env::consts::OS);
                eprintln!("⚠️ Falling back to stdout/stderr logging");
                // Fall back to stdout logging
                tracing_subscriber::fmt::init();
                tracing::warn!("File logging unavailable, using stdout/stderr only");
            }
        }
    } else {
        // File logging disabled, use stdout only
        tracing_subscriber::fmt::init();
        tracing::info!("File logging disabled, using stdout only");
    }
    
    let vfs_state = VfsStateWrapper::new();
    let media_state = MediaStateWrapper::new();
    
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(vfs_state)
        .manage(media_state)
        .setup(|app| {
            // Media service will be initialized lazily on first use
            // This avoids needing Tokio runtime in setup callback
            // GPU metrics polling will start when user opens metrics page
            
            // Setup tray icon click handler
            let app_handle = app.handle().clone();
            if let Some(tray) = app.tray_by_id("main") {
                tray.on_tray_icon_event(move |_tray, event| {
                    if let TrayIconEvent::Click { .. } = event {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                });
            }
            
            // Show window on startup for dev mode
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            show_window,
            hide_window,
            toggle_window,
            toggle_devtools,
            open_devtools,
            close_devtools,
            // GPU & System metrics commands
            commands::get_gpu_info,
            commands::get_gpu_metrics,
            commands::start_gpu_polling,
            commands::stop_gpu_polling,
            commands::get_system_info,
            commands::get_all_metrics,
            commands::start_model,
            commands::stop_model,
            commands::get_model_status,
            // AI Dependencies Installation commands
            commands::detect_platform,
            commands::check_docker_installed,
            commands::check_docker_running,
            commands::check_ollama_installed,
            commands::check_ollama_running,
            commands::check_ffmpeg_installed,
            commands::get_ollama_install_instructions,
            commands::get_ffmpeg_install_instructions,
            commands::install_docker,
            commands::install_ollama,
            commands::install_ffmpeg,
            commands::install_whisper_cpp,
            commands::check_whisper_cpp_installed,
            commands::install_all_ai_dependencies,
            // Ollama model management commands
            commands::ollama_list,
            commands::ollama_pull,
            commands::ollama_ps,
            commands::ollama_run,
            commands::ollama_stop,
            commands::ollama_delete,
            commands::ollama_serve,
            // AI Resource management commands
            commands::save_transcoding_resource_limits,
            commands::load_transcoding_resource_limits,
            commands::save_auto_tagging_resource_limits,
            commands::load_auto_tagging_resource_limits,
            // Token management commands
            commands::get_token_balance,
            commands::consume_tokens,
            commands::get_token_plans,
            commands::purchase_token_plan,
            // Logging commands
            commands::get_logs,
            commands::clear_logs,
            commands::get_log_file_path,
            // Settings commands
            commands::get_settings,
            commands::get_logging_settings,
            commands::update_logging_settings,
            commands::get_ui_settings,
            commands::update_ui_settings,
            commands::reset_settings,
            // VFS Clean Architecture commands
            vfs::commands::vfs_init,
            vfs::commands::vfs_list_sources,
            vfs::commands::vfs_test_s3_connection,
            vfs::commands::vfs_add_source,
            vfs::commands::vfs_remove_source,
            vfs::commands::vfs_refresh_s3_credentials,
            vfs::commands::vfs_update_s3_credentials,
            vfs::commands::vfs_register_s3_source,
            vfs::commands::vfs_setup_s3_testing,
            vfs::commands::vfs_mount_local,
            vfs::commands::vfs_eject,
            vfs::commands::vfs_list_files,
            vfs::commands::vfs_warm_file,
            vfs::commands::vfs_transcode_video,
            vfs::commands::vfs_cache_stats,
            vfs::commands::vfs_clear_cache,
            // VFS POSIX file operations
            vfs::commands::vfs_mkdir,
            vfs::commands::vfs_mkdir_p,
            vfs::commands::vfs_rmdir,
            vfs::commands::vfs_rename,
            vfs::commands::vfs_copy,
            vfs::commands::vfs_move,
            vfs::commands::vfs_batch_move,
            vfs::commands::vfs_delete,
            vfs::commands::vfs_delete_recursive,
            vfs::commands::vfs_batch_delete,
            vfs::commands::vfs_chmod,
            vfs::commands::vfs_stat,
            vfs::commands::vfs_get_folder_size,
            vfs::commands::vfs_touch,
            vfs::commands::vfs_exists,
            vfs::commands::vfs_read_text,
            vfs::commands::vfs_read_file_bytes,
            vfs::commands::vfs_download_file,
            vfs::commands::vfs_download_to_downloads,
            vfs::commands::vfs_write_text,
            vfs::commands::vfs_append_text,
            // VFS Clipboard commands
            vfs::commands::vfs_clipboard_copy,
            vfs::commands::vfs_clipboard_cut,
            vfs::commands::vfs_clipboard_copy_native,
            vfs::commands::vfs_clipboard_copy_for_native,
            vfs::commands::vfs_clipboard_get,
            vfs::commands::vfs_clipboard_get_content,
            vfs::commands::vfs_clipboard_has_files,
            vfs::commands::vfs_clipboard_clear,
            vfs::commands::vfs_clipboard_paste_to_vfs,
            vfs::commands::vfs_clipboard_paste_to_native,
            vfs::commands::vfs_clipboard_read_native,
            vfs::commands::vfs_clipboard_write_native,
            // VFS Tags & Favorites commands
            vfs::commands::vfs_get_metadata,
            vfs::commands::vfs_add_tag,
            vfs::commands::vfs_remove_tag,
            vfs::commands::vfs_toggle_favorite,
            vfs::commands::vfs_set_favorite,
            vfs::commands::vfs_set_color_label,
            vfs::commands::vfs_set_rating,
            vfs::commands::vfs_set_comment,
            vfs::commands::vfs_list_favorites,
            vfs::commands::vfs_list_by_tag,
            vfs::commands::vfs_list_by_color,
            vfs::commands::vfs_list_all_tags,
            // VFS Cross-Storage commands
            vfs::commands::vfs_copy_to_source,
            vfs::commands::vfs_move_to_source,
            vfs::commands::vfs_get_transfer_targets,
            vfs::commands::vfs_batch_copy_to_source,
            vfs::commands::vfs_batch_move_to_source,
            // VFS Sync commands
            vfs::commands::vfs_sync,
            vfs::commands::vfs_get_sync_targets,
            vfs::commands::vfs_change_tier,
            vfs::commands::vfs_sync_to_tier,
            vfs::commands::vfs_get_tier_targets,
            vfs::commands::vfs_check_nvme_cache,
            vfs::commands::vfs_set_tags,
            vfs::commands::vfs_reveal_in_finder,
            // VFS Open file commands
            vfs::commands::vfs_open_file,
            vfs::commands::vfs_open_file_with,
            vfs::commands::vfs_get_apps_for_file,
            vfs::commands::vfs_get_os_preferences,
            vfs::commands::vfs_get_thumbnail,
            vfs::commands::vfs_get_thumbnails_batch,
            // VFS Transcription commands
            vfs::commands::vfs_start_transcription,
            vfs::commands::vfs_stop_transcription,
            // VFS Video Streaming commands
            vfs::commands::vfs_get_stream_url,
            vfs::commands::vfs_get_file_range,
            vfs::commands::vfs_get_transcription_status,
            vfs::commands::vfs_get_transcription_segments,
            vfs::commands::vfs_transcribe_file,
            vfs::commands::vfs_save_transcription,
            vfs::commands::vfs_get_transcription_models,
            vfs::commands::vfs_is_transcription_available,
            vfs::commands::vfs_list_transcriptions,
            vfs::commands::vfs_get_transcription_progress,
            // Multipart upload commands
            vfs::commands::vfs_start_multipart_upload,
            vfs::commands::vfs_upload_folder,
            vfs::commands::vfs_batch_upload,
            vfs::commands::vfs_is_directory,
            vfs::commands::vfs_get_upload_progress,
            vfs::commands::vfs_resume_upload,
            vfs::commands::vfs_pause_upload,
            vfs::commands::vfs_cancel_upload,
            vfs::commands::vfs_list_uploads,
            vfs::commands::vfs_remove_upload,
            vfs::commands::vfs_list_downloads,
            vfs::commands::vfs_get_download_progress,
            vfs::commands::vfs_resume_download,
            vfs::commands::vfs_pause_download,
            vfs::commands::vfs_cancel_download,
            vfs::commands::vfs_cleanup_old_uploads,
            vfs::commands::vfs_list_operations,
            vfs::commands::vfs_get_audit_history,
            vfs::commands::vfs_get_organization_audit,
            vfs::commands::vfs_cancel_operation,
            vfs::commands::vfs_restart_operation,
            vfs::commands::vfs_delete_operation,
            vfs::commands::vfs_clear_all_operations,
            // Audit log commands (new)
            vfs::commands::vfs_get_user_audit_log,
            vfs::commands::vfs_get_organization_audit_log,
            vfs::commands::vfs_get_all_audit_log,
            // VFS Model Management commands
            vfs::commands::vfs_list_models,
            vfs::commands::vfs_list_models_by_category,
            vfs::commands::vfs_get_model,
            vfs::commands::vfs_install_model,
            vfs::commands::vfs_uninstall_model,
            vfs::commands::vfs_start_model,
            vfs::commands::vfs_stop_model,
            vfs::commands::vfs_get_model_categories,
            // VFS Auto Operations commands
            vfs::commands::vfs_auto_tag_file,
            vfs::commands::vfs_auto_transcode,
            vfs::commands::vfs_ensure_models_running,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
