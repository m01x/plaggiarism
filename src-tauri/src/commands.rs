use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

use crate::robocopy::{
    build_scan_args, cancel_core, run_robocopy_core, CopyErrorEvent, FileCompletedEvent,
    FileProgressEvent, RobocopyParams, RobocopyResult, ScanCounters,
};
use crate::state::{RobocopyState, ScanState, SharedScanReader};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub file_count: u64,
    /// Bytes a copiar (columna "Copiado" de la fila "Bytes" del resumen, con `/BYTES`).
    pub total_bytes: u64,
    pub extras_count: u64,
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
    let args = build_scan_args(
        &params.origen,
        &params.destino,
        &params.modo,
        &params.excluir,
    );

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

    let mut counters = ScanCounters::default();

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
            counters.process_line(&line);
        }
        Ok::<(), String>(())
    };

    match timeout(std::time::Duration::from_secs(8), scan_future).await {
        Ok(Ok(())) => {
            // Scan terminó dentro de 8s.
            let _ = child.wait().await;
            Ok(ScanResult {
                file_count: counters.file_count,
                total_bytes: counters.total_bytes,
                extras_count: counters.extras_count,
                slow: false,
            })
        }
        Ok(Err(e)) => Err(format!("error de scan: {e}")),
        Err(_) => {
            // Timeout: el proceso sigue corriendo. Guardamos child + reader + conteo
            // parcial para que poll_scan continúe drenando stdout.
            let file_count = counters.file_count;
            let total_bytes = counters.total_bytes;
            let extras_count = counters.extras_count;
            let state = ScanState {
                child: Some(child),
                reader: Some(reader),
                counters,
                done: false,
            };
            *scan_holder.state.lock().map_err(|e| e.to_string())? = Some(state);
            Ok(ScanResult {
                file_count,
                total_bytes,
                extras_count,
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
            file_count: state.counters.file_count,
            total_bytes: state.counters.total_bytes,
            extras_count: state.counters.extras_count,
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
        state.counters.process_line(&line);
    }

    // El scan terminó — esperar al proceso para liberar recursos.
    if let Some(child) = state.child.as_mut() {
        let _ = child.wait().await;
    }

    let result = ScanResult {
        file_count: state.counters.file_count,
        total_bytes: state.counters.total_bytes,
        extras_count: state.counters.extras_count,
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
    let app_progress = app.clone();
    let on_file_progress = move |e: FileProgressEvent| {
        let _ = app_progress.emit("file_progress", e);
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
        on_file_progress,
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

#[tauri::command]
pub fn disk_free_space(path: String) -> Result<u64, String> {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    // Ruta a UTF-16 terminada en null.
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

    let mut free_available: u64 = 0;
    let mut total: u64 = 0;
    let mut total_free: u64 = 0;

    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_available,
            &mut total,
            &mut total_free,
        )
    };

    if ok == 0 {
        return Err(format!("No se pudo obtener el espacio libre de {path}"));
    }

    // Bytes disponibles para el usuario actual (contempla cuotas).
    Ok(free_available)
}

#[tauri::command]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| format!("No se pudo escribir {path}: {e}"))
}

#[tauri::command]
pub fn read_text_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("No se pudo leer {path}: {e}"))
}
