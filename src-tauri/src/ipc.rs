use std::str::FromStr;

use crate::{gossip::NodeId, state::AppContext, utils::AppStore};
use anyhow::anyhow;
use iroh_docs::DocTicket;

#[tauri::command]
/// Create a new room and return the information required to send
/// an out-of-band Join Code to others to connect.
pub async fn create_room(
    nickname: String,
    state: tauri::State<'_, AppContext>,
    app: tauri::AppHandle,
) -> tauri::Result<String> {
    // Leave any existing room first
    leave_room(state.clone(), app.clone()).await?;

    let store = AppStore::acquire(&app)?;
    store.set_nickname(&nickname)?;

    // Store the active channel info
    state.start_channel(None, &app, &nickname).await?;

    // Get the topic_id from the established channel for logging
    let topic_id_str = state.get_topic_id().await?;

    tracing::info!("Created and joined room: {}", topic_id_str);

    // Generate ticket string from the Channel instance to be shared
    let ticket_token = state.generate_ticket(TicketOpts::all()).await?;

    Ok(ticket_token)
}

#[tauri::command]
/// Join an existing room
pub async fn join_room(
    ticket: String,
    nickname: String,
    state: tauri::State<'_, AppContext>,
    app: tauri::AppHandle,
) -> tauri::Result<()> {
    // Leave any existing room first
    leave_room(state.clone(), app.clone()).await?;

    let ticket =
        DocTicket::from_str(&ticket).map_err(|e| anyhow!("Invalid activity ticket: {}", e))?;
    // Store the active channel info
    state.start_channel(Some(ticket), &app, &nickname).await?;

    // Get the topic_id from the established channel for logging
    let topic_id_str = state.get_topic_id().await?;

    tracing::info!(
        "Active channel SET in join_room for topic: {}",
        topic_id_str
    );
    AppStore::acquire(&app)?.set_nickname(&nickname)?;
    tracing::info!("Joined room: {}", topic_id_str);
    Ok(())
}

#[tauri::command]
/// Send a message to the room
pub async fn send_message(
    message: String,
    state: tauri::State<'_, AppContext>,
    _app: tauri::AppHandle, // Marked as unused, can be removed if not needed by Tauri
) -> tauri::Result<()> {
    let sender = state.get_sender().await?;
    sender.send(message).await?;
    Ok(())
}

#[tauri::command]
/// Set a new nickname for this node.
pub async fn set_nickname(nickname: String, app: tauri::AppHandle) -> tauri::Result<()> {
    tracing::info!("Nickname set to: {}", &nickname);
    AppStore::acquire(&app)?.set_nickname(&nickname)?;
    Ok(())
}

#[tauri::command]
/// Get the stored nickname for this node.
pub async fn get_nickname(app: tauri::AppHandle) -> tauri::Result<Option<String>> {
    Ok(AppStore::acquire(&app)?.get_nickname())
}

#[tauri::command]
/// Get the stored room ticket string
pub async fn get_latest_ticket(
    state: tauri::State<'_, AppContext>,
) -> tauri::Result<Option<String>> {
    let ticket_guard = state.latest_ticket.lock().await;
    Ok(ticket_guard.clone())
}

#[tauri::command]
/// Leave the currently joined room
pub async fn leave_room(
    state: tauri::State<'_, AppContext>,
    _app: tauri::AppHandle,
) -> tauri::Result<()> {
    if let Some(id) = state.drop_channel().await? {
        tracing::info!("Left room: {}", id);
    };
    Ok(())
}

#[tauri::command]
/// Returns the node id of this node
pub async fn get_node_id(state: tauri::State<'_, AppContext>) -> tauri::Result<NodeId> {
    let node = state.node.lock().await;
    node.as_ref()
        .map(|chat_node| chat_node.node_id())
        .ok_or(anyhow!("Node not initialized").into())
}
