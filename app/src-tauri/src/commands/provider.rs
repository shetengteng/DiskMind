//! AI Provider 的增删改查。前端 Settings → AI 页直接消费。

use tauri::State;

use crate::db;
use crate::state::ScanState;

#[tauri::command]
pub fn provider_list(state: State<'_, ScanState>) -> Result<Vec<db::Provider>, String> {
    state.db.provider_list().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn provider_save(
    provider: db::ProviderUpsert,
    state: State<'_, ScanState>,
) -> Result<db::Provider, String> {
    state.db.provider_upsert(provider).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn provider_delete(id: String, state: State<'_, ScanState>) -> Result<u64, String> {
    state.db.provider_delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn provider_set_default(id: String, state: State<'_, ScanState>) -> Result<u64, String> {
    state
        .db
        .provider_set_default(&id)
        .map_err(|e| e.to_string())
}
