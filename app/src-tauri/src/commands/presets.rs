use serde::Serialize;
use tauri::State;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct PresetSummary {
    pub id: String,
    pub name: String,
}

#[tauri::command]
pub fn list_presets(state: State<'_, AppState>) -> Vec<PresetSummary> {
    state.presets.iter().map(|p| PresetSummary { id: p.id.clone(), name: p.name.clone() }).collect()
}
