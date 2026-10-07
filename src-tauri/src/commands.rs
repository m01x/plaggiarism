use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

use crate::robocopy::{
    build_scan_args, cancel_core, parse_file_completed, parse_size_bytes, parse_summary_line,
    run_robocopy_core, CopyErrorEvent, FileCompletedEvent, RobocopyParams, RobocopyResult,
};
use crate::state::{RobocopyState, ScanState, SharedScanReader};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub file_count: u64,
    pub total_bytes: u64,
    pub slow: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathsValidation {
    pub origen_ok: bool,
    pub destino_ok: bool,
}

#[derive(Default)]
pub struct ScanHolder {
    pub state: SharedScanReader,
}

#[tauri::command]
pub fn validate_paths(origen: String, destino: String) -> PathsValidation {
    let origen_ok = Path::new(&origen).exists() && Path::new(&origen).is_dir();
    let destino_ok = Path::new(&destino).exists() && Path::new(&destino).is_dir();
    PathsValidation {
        origen_ok,
        destino_ok,
    }
}

#[tauri::command]
pub async fn scan_robocopy(
    params: RobocopyParams,
    scan_holder: State<'_, ScanHolder>,
) -> Result<ScanResult, String> {
    let args = build_scan_args(&params.origen, &params.destino);

    let mut child = Command::new("robocopy")
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("No se pudo iniciar robocopy: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "robocopy no produjo stdout".to_string())?;
    let mut reader = BufReader::new(stdout);

    let mut file_count: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut summary_rows_seen: u32 = 0;

    let scan_future = async {
        let mut buf: Vec<u8> = Vec::new();
        loop {
            buf.clear();
            let n = reader
                .read_until(b'\n', &mut buf)
                .await
                .map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            let line = encoding_rs::WINDOWS_1252.decode(&buf).0;
            if let Some((_, copiados, _, _, _, _)) = parse_summary_line(&line) {
                // Robocopy imprime Directorios (fila 0), Archivos (fila 1),
                // Bytes (no parsea como u64), Tiempo (no parsea).
                // Usamos la fila 1 (Archivos) para el conteo de archivos.
                summary_rows_seen += 1;
                if summary_rows_seen == 2 {
                    file_count = copiados;
                }
            } else if let Some(name) = parse_file_completed(&line) {
                file_count = file_count.saturating_add(1);
                if let Some(b) = parse_size_bytes(&line) {
                    total_bytes = total_bytes.saturating_add(b);
                }
                let _ = name;
            }
        }
        Ok::<(), String>(())
    };

    match timeout(std::time::Duration::from_secs(8), scan_future).await {
        Ok(Ok(())) => {
            // Scan terminó dentro de 8s.
            let _ = child.wait().await;
            Ok(ScanResult {
                file_count,
                total_bytes,
                slow: false,
            })
        }
        Ok(Err(e)) => Err(format!("error de scan: {e}")),
        Err(_) => {
            // Timeout: el proceso sigue corriendo. Guardamos child + reader + conteo
            // parcial para que poll_scan continúe drenando stdout.
            let state = ScanState {
                child: Some(child),
                reader: Some(reader),
                file_count,
                total_bytes,
                done: false,
                summary_rows_seen,
            };
            *scan_holder.state.lock().map_err(|e| e.to_string())? = Some(state);
            Ok(ScanResult {
                file_count,
                total_bytes,
                slow: true,
            })
        }
    }
}

#[tauri::command]
pub async fn poll_scan(scan_holder: State<'_, ScanHolder>) -> Result<Option<ScanResult>, String> {
    // Sacar el ScanState fuera del lock para no mantenerlo a través de .await.
    let mut state_opt = {
        let mut guard = scan_holder.state.lock().map_err(|e| e.to_string())?;
        guard.take()
    };

    let Some(state) = state_opt.as_mut() else {
        return Ok(None);
    };

    if state.done {
        let result = ScanResult {
            file_count: state.file_count,
            total_bytes: state.total_bytes,
            slow: false,
        };
        // state ya consumido (Option::take arriba). Confirmar limpieza.
        state_opt.take();
        return Ok(Some(result));
    }

    let Some(reader) = state.reader.as_mut() else {
        // No hay reader — devolverlo al lock para reintentar luego.
        let recovered = state_opt.take();
        let mut guard = scan_holder.state.lock().map_err(|e| e.to_string())?;
        *guard = recovered;
        return Ok(None);
    };

    let mut buf: Vec<u8> = Vec::new();
    loop {
        buf.clear();
        let n = reader
            .read_until(b'\n', &mut buf)
            .await
            .map_err(|e| e.to_string())?;
        if n == 0 {
            state.done = true;
            break;
        }
        let line = encoding_rs::WINDOWS_1252.decode(&buf).0;
        if let Some((_, copiados, _, _, _, _)) = parse_summary_line(&line) {
            // Misma lógica que scan_robocopy: fila 1 = Archivos.
            state.summary_rows_seen += 1;
            if state.summary_rows_seen == 2 {
                state.file_count = copiados;
            }
        } else if parse_file_completed(&line).is_some() {
            state.file_count = state.file_count.saturating_add(1);
            if let Some(b) = parse_size_bytes(&line) {
                state.total_bytes = state.total_bytes.saturating_add(b);
            }
        }
    }

    // El scan terminó — esperar al proceso para liberar recursos.
    if let Some(child) = state.child.as_mut() {
        let _ = child.wait().await;
    }

    let result = ScanResult {
        file_count: state.file_count,
        total_bytes: state.total_bytes,
        slow: false,
    };
    state_opt.take();
    Ok(Some(result))
}

#[tauri::command]
pub async fn run_robocopy(
    app: AppHandle,
    params: RobocopyParams,
    total_files: u64,
    robo_state: State<'_, RobocopyState>,
) -> Result<RobocopyResult, String> {
    let child_slot = robo_state.child.clone();

    let app_file = app.clone();
    let on_file_completed = move |e: FileCompletedEvent| {
        let _ = app_file.emit("file_completed", e);
    };
    let app_error = app.clone();
    let on_copy_error = move |e: CopyErrorEvent| {
        let _ = app_error.emit("copy_error", e);
    };

    let result = run_robocopy_core(
        &params,
        total_files,
        &child_slot,
        on_file_completed,
        on_copy_error,
    )
    .await?;

    let _ = app.emit("copy_done", result.clone());
    Ok(result)
}

#[tauri::command]
pub async fn cancel_robocopy(robo_state: State<'_, RobocopyState>) -> Result<(), String> {
    cancel_core(&robo_state.child).await
}
