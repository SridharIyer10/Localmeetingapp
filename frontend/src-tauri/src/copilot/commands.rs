//! Tauri commands for co-pilot question sets, manual question tagging, the
//! post-meeting question summary and local file saving. Analysis itself lives in
//! `copilot::analysis`; these commands own persistence and the audit trail.

use crate::copilot::analysis::{
    analysis_status, run_analysis, AnalysisConfig, AnalysisReport, AnalysisStatus,
};
use crate::database::models_copilot::{
    BankQuestionModel, MeetingQuestionView, QuestionBankGroupModel, TranscriptSegmentModel,
};
use crate::database::repositories::copilot::{CopilotRepository, NewQuestion};
use crate::state::AppState;
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_dialog::DialogExt;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ---- meeting questions ------------------------------------------------------

#[tauri::command]
pub async fn copilot_list_questions(
    state: State<'_, AppState>,
    meeting_id: String,
) -> Result<Vec<MeetingQuestionView>, String> {
    CopilotRepository::list_questions(state.db_manager.pool(), &meeting_id)
        .await
        .map(|rows| rows.into_iter().map(MeetingQuestionView::from).collect())
        .map_err(err)
}

/// Manual tag set by the interviewer during the call (`asked | not_asked | answered`, or `None` to clear).
/// Editable until the summary is generated.
#[tauri::command]
pub async fn copilot_set_question_tag(
    state: State<'_, AppState>,
    question_id: String,
    tag: Option<String>,
) -> Result<(), String> {
    CopilotRepository::set_user_tag(
        state.db_manager.pool(),
        &question_id,
        tag.as_deref(),
    )
    .await
    .map_err(err)
}

#[tauri::command]
pub async fn copilot_set_question_notes(
    state: State<'_, AppState>,
    question_id: String,
    notes: String,
) -> Result<(), String> {
    CopilotRepository::set_notes(state.db_manager.pool(), &question_id, &notes)
        .await
        .map_err(err)
}

/// Every automated verdict is user-correctable.
#[tauri::command]
pub async fn copilot_override_verdict(
    state: State<'_, AppState>,
    question_id: String,
    status: String,
    rating: Option<String>,
    extracted_answer: Option<String>,
) -> Result<(), String> {
    CopilotRepository::override_verdict(
        state.db_manager.pool(),
        &question_id,
        &status,
        rating.as_deref(),
        extracted_answer.as_deref(),
    )
    .await
    .map_err(err)
}

#[tauri::command]
pub async fn copilot_list_segments(
    state: State<'_, AppState>,
    meeting_id: String,
) -> Result<Vec<TranscriptSegmentModel>, String> {
    CopilotRepository::list_segments(state.db_manager.pool(), &meeting_id)
        .await
        .map_err(err)
}

/// Begin a co-pilot session: create the `draft-*` meeting holding a copy of the
/// chosen question set's questions, in set order. Safe to call again on retry.
#[tauri::command]
pub async fn copilot_begin_session(
    state: State<'_, AppState>,
    draft_id: String,
    set_id: String,
) -> Result<Vec<MeetingQuestionView>, String> {
    let pool = state.db_manager.pool();
    let ids: Vec<String> = CopilotRepository::bank_list(pool)
        .await
        .map_err(err)?
        .into_iter()
        .filter(|i| i.bank_id.as_deref() == Some(set_id.as_str()))
        .map(|i| i.id)
        .collect();
    if ids.is_empty() {
        return Err("this question set has no questions".into());
    }
    CopilotRepository::begin_draft(pool, &draft_id, &ids).await.map_err(err)?;
    CopilotRepository::list_questions(pool, &draft_id)
        .await
        .map(|rows| rows.into_iter().map(MeetingQuestionView::from).collect())
        .map_err(err)
}

/// Attach the saved recording (its own meeting row) to the co-pilot meeting whose
/// questions were prepared before recording. Returns the transcript segment count.
#[tauri::command]
pub async fn copilot_finalize_recording(
    state: State<'_, AppState>,
    copilot_meeting_id: String,
    saved_meeting_id: String,
) -> Result<u64, String> {
    CopilotRepository::finalize_recording(
        state.db_manager.pool(),
        &copilot_meeting_id,
        &saved_meeting_id,
    )
    .await
    .map_err(err)
}

// ---- post-meeting analysis --------------------------------------------------

/// Question summary (Part A): status + reasoned answer + evidence for every question,
/// guided by the interviewer's manual tags.
#[tauri::command]
pub async fn copilot_run_analysis<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    meeting_id: String,
) -> Result<AnalysisReport, String> {
    let data_dir = app.path().app_data_dir().ok();
    let config = AnalysisConfig {
        embed_model_dir: data_dir.as_ref().map(|d| d.join("models").join("embedding")),
        app_data_dir: data_dir,
        ..Default::default()
    };
    run_analysis(state.db_manager.pool(), &meeting_id, config).await
}

#[tauri::command]
pub async fn copilot_analysis_status(
    state: State<'_, AppState>,
    meeting_id: String,
) -> Result<AnalysisStatus, String> {
    analysis_status(state.db_manager.pool(), &meeting_id).await
}

// ---- question sets ----------------------------------------------------------

#[tauri::command]
pub async fn question_bank_group_list(
    state: State<'_, AppState>,
) -> Result<Vec<QuestionBankGroupModel>, String> {
    CopilotRepository::group_list(state.db_manager.pool()).await.map_err(err)
}

#[tauri::command]
pub async fn question_bank_group_create(
    state: State<'_, AppState>,
    name: String,
) -> Result<String, String> {
    CopilotRepository::group_create(state.db_manager.pool(), &name).await.map_err(err)
}

#[tauri::command]
pub async fn question_bank_group_rename(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<(), String> {
    CopilotRepository::group_rename(state.db_manager.pool(), &id, &name).await.map_err(err)
}

#[tauri::command]
pub async fn question_bank_group_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    CopilotRepository::group_delete(state.db_manager.pool(), &id).await.map_err(err)
}

#[tauri::command]
pub async fn question_bank_list(
    state: State<'_, AppState>,
) -> Result<Vec<BankQuestionModel>, String> {
    CopilotRepository::bank_list(state.db_manager.pool()).await.map_err(err)
}

#[tauri::command]
pub async fn question_bank_add(
    state: State<'_, AppState>,
    question: String,
    sample_questions: Vec<String>,
    sample_answers: Vec<String>,
    bank_id: Option<String>,
) -> Result<String, String> {
    CopilotRepository::bank_add(
        state.db_manager.pool(),
        &question,
        &sample_questions,
        &sample_answers,
        bank_id.as_deref(),
    )
    .await
    .map_err(err)
}

#[tauri::command]
pub async fn question_bank_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    CopilotRepository::bank_delete(state.db_manager.pool(), &id).await.map_err(err)
}

#[derive(Serialize)]
pub struct SetImportResult {
    pub id: String,
    /// true: a new set was created; false: an existing set's questions were replaced.
    pub created: bool,
    pub count: usize,
}

/// CSV import target: create the set, or replace its questions if it exists.
#[tauri::command]
pub async fn question_set_import(
    state: State<'_, AppState>,
    name: String,
    questions: Vec<NewQuestion>,
) -> Result<SetImportResult, String> {
    let count = questions.len();
    CopilotRepository::group_import(state.db_manager.pool(), &name, &questions)
        .await
        .map(|(id, created)| SetImportResult { id, created, count })
        .map_err(err)
}

// ---- export -----------------------------------------------------------------

/// Ask the user where to save a text file (native dialog) and write it.
/// Returns the chosen path, or `None` if the dialog was cancelled.
#[tauri::command]
pub async fn save_text_file<R: Runtime>(
    app: AppHandle<R>,
    suggested_name: String,
    content: String,
) -> Result<Option<String>, String> {
    let safe: String = suggested_name
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') { c } else { '_' })
        .collect();
    let ext = std::path::Path::new(&safe)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("txt")
        .to_lowercase();
    let (filter_name, filter_exts): (&str, &[&str]) = match ext.as_str() {
        "csv" => ("CSV", &["csv"]),
        "md" => ("Markdown", &["md", "txt"]),
        _ => ("Text", &["txt", "md"]),
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(&safe)
        .add_filter(filter_name, filter_exts)
        .save_file(move |p| {
            let _ = tx.send(p);
        });
    let Some(path) = rx.await.map_err(err)? else {
        return Ok(None);
    };
    let path = path.into_path().map_err(err)?;
    tokio::fs::write(&path, content).await.map_err(err)?;
    Ok(Some(path.display().to_string()))
}
