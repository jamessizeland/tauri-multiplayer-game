use anyhow::anyhow;
use tauri::Manager as _;
use utils::AppStore;

use crate::state::AppContext;

mod game;
mod gossip;
mod ipc;
mod state;
mod utils;

/// Initialize the Application Context from disk.
async fn init_context(app: tauri::AppHandle) -> tauri::Result<()> {
    let data_root = app
        .path()
        .app_data_dir()
        .map_err(|_| anyhow!("can't get application data directory"))?
        .join("iroh_data");

    // Spawn the Iroh node
    let key = AppStore::acquire(&app)?.get_secret_key()?;
    let node = gossip::GossipNode::spawn(Some(key), data_root)
        .await
        .map_err(|e| anyhow!("Failed to spawn node: {}", e))?;

    app.manage(AppContext::new(node));

    let state = app.state::<state::AppContext>();
    state.drop_channel().await?; // Reset active channel on init.

    tracing::info!("Iroh node initialized.");
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .pretty()
        .with_ansi(false)
        .init();

    tracing::info!("Starting app");

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(debug_assertions)] // only include this code on debug builds
            app.get_webview_window("main").unwrap().open_devtools();
            let handle = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                init_context(handle).await.unwrap();
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::create_room,
            ipc::join_room,
            ipc::send_message,
            ipc::leave_room,
            ipc::get_latest_ticket,
            ipc::get_node_id,
            ipc::set_nickname,
            ipc::get_nickname,
            ipc::get_message_log,
            ipc::get_peers,
        ])
        .run(tauri::generate_context!()) // Run the Tauri application
        .expect("error while running tauri application");
}
