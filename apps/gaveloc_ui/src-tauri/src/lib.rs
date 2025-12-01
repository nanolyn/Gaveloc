mod commands;
mod state;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize application state
    let app_state = AppState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health_check,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::validate_game_path,
            commands::settings::detect_game_install,
            commands::settings::get_default_install_path,
            commands::accounts::list_accounts,
            commands::accounts::get_default_account,
            commands::accounts::add_account,
            commands::accounts::update_account,
            commands::accounts::remove_account,
            commands::accounts::set_default_account,
            commands::accounts::has_stored_password,
            commands::accounts::store_password,
            commands::accounts::delete_password,
            commands::auth::login,
            commands::auth::login_with_cached_session,
            commands::auth::logout,
            commands::auth::get_stored_password,
            commands::auth::get_session_status,
            commands::auth::start_otp_listener,
            commands::auth::stop_otp_listener,
            commands::auth::is_otp_listener_running,
            commands::version::init_version_repo,
            commands::version::get_game_versions,
            commands::version::check_boot_updates,
            commands::version::check_game_updates,
            commands::patching::start_boot_patch,
            commands::patching::start_game_patch,
            commands::patching::cancel_patch,
            commands::patching::get_patch_status,
            commands::integrity::verify_integrity,
            commands::integrity::repair_files,
            commands::integrity::cancel_integrity_check,
            commands::integrity::get_integrity_status,
            commands::runners::list_runners,
            commands::runners::validate_runner,
            commands::runners::get_selected_runner,
            commands::runners::select_runner,
            commands::launcher::preflight_check,
            commands::launcher::launch_game,
            commands::launcher::get_launch_status,
            commands::news::get_headlines,
            commands::news::get_banners,
            commands::news::get_news_article,
            commands::news::proxy_image,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
