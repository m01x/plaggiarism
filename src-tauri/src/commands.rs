use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::timeout;

use crate::robocopy::{
    build_run_args, build_scan_args, map_exit_code, parse_file_completed, parse_size_bytes,
    parse_summary_line, RobocopyResult,
};
use crate::state::{RobocopyState, ScanState, SharedScanReader};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RobocopyParams {
    pub origen: String,
    pub destino: String,
    pub modo: String,
    pub excluir: Vec<String>,
}

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

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileCompletedEvent {
    pub remaining: u64,
    pub file_name: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CopyErrorEvent {
    pub file_name: String,
    pub error_code: i32,
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
        let mut buf = String::new();
        loop {
            buf.clear();
            let n = reader
                .read_line(&mut buf)
                .await
                .map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            if let Some((_, copiados, _, _, _, _)) = parse_summary_line(&buf) {
                // Robocopy imprime Directorios (fila 0), Archivos (fila 1),
                // Bytes (no parsea como u64), Tiempo (no parsea).
                // Usamos la fila 1 (Archivos) para el conteo de archivos.
                summary_rows_seen += 1;
                if summary_rows_seen == 2 {
                    file_count = copiados;
                }
            } else if let Some(name) = parse_file_completed(&buf) {
                file_count = file_count.saturating_add(1);
                if let Some(b) = parse_size_bytes(&buf) {
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

    let mut buf = String::new();
    loop {
        buf.clear();
        let n = reader
            .read_line(&mut buf)
            .await
            .map_err(|e| e.to_string())?;
        if n == 0 {
            state.done = true;
            break;
        }
        if let Some((_, copiados, _, _, _, _)) = parse_summary_line(&buf) {
            // Misma lógica que scan_robocopy: fila 1 = Archivos.
            state.summary_rows_seen += 1;
            if state.summary_rows_seen == 2 {
                state.file_count = copiados;
            }
        } else if parse_file_completed(&buf).is_some() {
            state.file_count = state.file_count.saturating_add(1);
            if let Some(b) = parse_size_bytes(&buf) {
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
    let args = build_run_args(
        &params.origen,
        &params.destino,
        &params.modo,
        &params.excluir,
    );
    let start_total = total_files;

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

    {
        let mut guard = robo_state.child.lock().map_err(|e| e.to_string())?;
        *guard = Some(child);
    }

    let start = Instant::now();
    let mut remaining = start_total;
    let mut failed_files: Vec<String> = Vec::new();
    let mut last_file_for_error: Option<String> = None;

    let mut buf = String::new();
    loop {
        buf.clear();
        let n = reader
            .read_line(&mut buf)
            .await
            .map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }

        if let Some((_, _copiados, _skipped, _, _failed, _)) = parse_summary_line(&buf) {
            // El resumen final es referencia; los totales emitidos en vivo
            // son los que utiliza el frontend para el contador.
            continue;
        }

        if let Some(name) = parse_file_completed(&buf) {
            remaining = remaining.saturating_sub(1);
            last_file_for_error = Some(name.clone());
            let _ = app.emit(
                "file_completed",
                FileCompletedEvent {
                    remaining,
                    file_name: name,
                },
            );
            continue;
        }

        // Detección de error por archivo: robocopy imprime `ERROR` o `FAILED`
        // seguido del motivo y de la ruta afectada.
        let up = buf.to_uppercase();
        if up.contains("ERROR") || up.contains("FAILED") {
            let name = last_file_for_error
                .clone()
                .unwrap_or_else(|| buf.trim().to_string());
            failed_files.push(name.clone());
            let _ = app.emit(
                "copy_error",
                CopyErrorEvent {
                    file_name: name,
                    error_code: 8,
                },
            );
        }
    }

    // Reobtener el child guardado para esperar su exit code.
    let mut taken: Option<Child> = {
        let mut guard = robo_state.child.lock().map_err(|e| e.to_string())?;
        guard.take()
    };

    let exit_code = if let Some(child) = taken.as_mut() {
        child
            .wait()
            .await
            .map_err(|e| format!("no se pudo esperar a robocopy: {e}"))?
            .code()
            .unwrap_or(-1)
    } else {
        -1
    };

    let status = map_exit_code(exit_code);
    let duration_secs = start.elapsed().as_secs_f64();

    let failed_count = failed_files.len() as u64;
    let result = RobocopyResult {
        status: status.clone(),
        copied: start_total
            .saturating_sub(remaining)
            .saturating_sub(failed_count),
        skipped: 0,
        failed: failed_count,
        failed_files,
        duration_secs,
    };
    let _ = app.emit("copy_done", result.clone());

    Ok(result)
}

#[tauri::command]
pub async fn cancel_robocopy(robo_state: State<'_, RobocopyState>) -> Result<(), String> {
    let mut taken: Option<Child> = {
        let mut guard = robo_state.child.lock().map_err(|e| e.to_string())?;
        guard.take()
    };
    if let Some(child) = taken.as_mut() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    Ok(())
}
