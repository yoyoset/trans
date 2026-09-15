use serde::Deserialize;
use tauri::State;
use trans_application::service::{CalibrateRequest, TranslateRequest, TranslationOutcome};
use trans_domain::conversation::Turn;
use trans_domain::glossary::GlossaryEntry;
use trans_domain::lang::{Direction, Tone};

use crate::error::CommandError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct TranslateArgs {
    pub direction: Direction,
    pub text: String,
    #[serde(default)]
    pub tone: Tone,
    /// 使用哪个区域预设的术语表；不传则不带术语表。
    #[serde(default)]
    pub preset_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CalibrateArgs {
    pub direction: Direction,
    pub source: String,
    pub draft: String,
    #[serde(default)]
    pub tone: Tone,
    #[serde(default)]
    pub context: Vec<Turn>,
    #[serde(default)]
    pub preset_id: Option<String>,
}

#[tauri::command]
pub async fn translate_text(
    request: TranslateArgs,
    state: State<'_, AppState>,
) -> Result<TranslationOutcome, CommandError> {
    let glossary = glossary_for(&state, request.preset_id.as_deref());
    let request = TranslateRequest { direction: request.direction, text: request.text, tone: request.tone, glossary };
    Ok(state.service.translate(request).await?)
}

#[tauri::command]
pub async fn calibrate_translation(
    request: CalibrateArgs,
    state: State<'_, AppState>,
) -> Result<TranslationOutcome, CommandError> {
    let glossary = glossary_for(&state, request.preset_id.as_deref());
    let request = CalibrateRequest {
        direction: request.direction,
        source: request.source,
        draft: request.draft,
        tone: request.tone,
        context: request.context,
        glossary,
    };
    Ok(state.service.calibrate(request).await?)
}

fn glossary_for(state: &AppState, preset_id: Option<&str>) -> Vec<GlossaryEntry> {
    preset_id.and_then(|id| state.presets.iter().find(|p| p.id == id)).map(|p| p.glossary.clone()).unwrap_or_default()
}
