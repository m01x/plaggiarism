use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use crate::state::SharedChild;

#[derive(Serialize, Deserialize, Clone)]
pub enum RobocopyStatus {
    NothingToDo,
    Success,
    ExtraFiles,
    SuccessWithExtra,
    Mismatch,
    SomeFailed,
    FatalError,
    Unknown(i32),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FailedFile {
    /// Ruta completa tal como la reporta robocopy (termina en `\` si es carpeta).
    pub file_name: String,
    pub error_code: i32,
    /// Descripción del error en el idioma del sistema ("Acceso denegado.").
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RobocopyResult {
    pub status: RobocopyStatus,
    pub copied: u64,
    pub skipped: u64,
    pub failed: u64,
    pub failed_files: Vec<FailedFile>,
    pub duration_secs: f64,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RobocopyParams {
    pub origen: String,
    pub destino: String,
    pub modo: String,
    pub excluir: Vec<String>,
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

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileProgressEvent {
    pub file_name: String,
    pub percent: u32,
}

/// El exit code de robocopy es una máscara de bits: 1 = copió, 2 = extras,
/// 4 = discrepancias, 8 = fallos, 16 = error fatal. Se combinan (9 = copió y
/// falló). Los negativos no vienen de robocopy (proceso cancelado).
pub fn map_exit_code(code: i32) -> RobocopyStatus {
    match code {
        c if c < 0 => RobocopyStatus::Unknown(c),
        c if c >= 16 => RobocopyStatus::FatalError,
        c if c & 8 != 0 => RobocopyStatus::SomeFailed,
        c if c & 4 != 0 => RobocopyStatus::Mismatch,
        0 => RobocopyStatus::NothingToDo,
        1 => RobocopyStatus::Success,
        2 => RobocopyStatus::ExtraFiles,
        _ => RobocopyStatus::SuccessWithExtra,
    }
}

pub fn build_run_args(origen: &str, destino: &str, modo: &str, excluir: &[String]) -> Vec<String> {
    let mut args = vec![origen.to_string(), destino.to_string()];

    match modo {
        "mirror" => {
            args.push("/MIR".to_string());
        }
        _ => {
            args.push("/E".to_string());
        }
    }

    args.push("/W:1".to_string());
    args.push("/R:1".to_string());
    args.push("/XJ".to_string());

    if !excluir.is_empty() {
        args.push("/XD".to_string());
        for p in excluir {
            args.push(p.clone());
        }
    }

    args
}

pub fn build_scan_args(origen: &str, destino: &str, modo: &str, excluir: &[String]) -> Vec<String> {
    let mut args = vec![origen.to_string(), destino.to_string(), "/L".to_string()];

    match modo {
        "mirror" => args.push("/MIR".to_string()),
        _ => args.push("/E".to_string()),
    }

    args.push("/W:1".to_string());
    args.push("/R:1".to_string());
    args.push("/XJ".to_string());
    args.push("/NFL".to_string());
    args.push("/NDL".to_string());
    args.push("/NJH".to_string());
    // Tamaños en bytes enteros: hace que la fila "Bytes" del resumen sea
    // parseable como enteros (para detección de espacio en destino).
    args.push("/BYTES".to_string());

    if !excluir.is_empty() {
        args.push("/XD".to_string());
        for p in excluir {
            args.push(p.clone());
        }
    }

    args
}

/// Extrae el porcentaje (0..100) de un segmento que termina en '%'.
pub fn parse_percent(segment: &str) -> Option<u32> {
    let s = segment.trim();
    if !s.ends_with('%') {
        return None;
    }
    let num = s[..s.len() - 1].trim();
    num.parse::<f64>().ok().map(|v| v.round() as u32)
}

/// Reconoce una línea de error real de robocopy y extrae código y ruta:
/// `2026/10/30 15:01:09 ERROR 5 (0x00000005) Copiando archivo C:\a\b.txt`
/// → `Some((5, "C:\a\b.txt"))`. Solo el texto tras el hex depende del idioma.
/// Si no encuentra ruta, devuelve ese texto. Exige el formato completo para no
/// confundir carpetas llamadas "errors" o la cabecera del resumen con un fallo.
pub fn parse_error_line(seg: &str) -> Option<(i32, String)> {
    let mut parts = seg.trim().splitn(6, ' ');
    let date = parts.next()?;
    let time = parts.next()?;
    let is_stamp = |s: &str, sep: char, len: usize| {
        s.len() == len && s.chars().all(|c| c.is_ascii_digit() || c == sep)
    };
    if !is_stamp(date, '/', 10) || !is_stamp(time, ':', 8) || parts.next()? != "ERROR" {
        return None;
    }
    let code = parts.next()?.parse::<i32>().ok()?;
    let hex = parts.next()?;
    if !hex.starts_with("(0x") || !hex.ends_with(')') {
        return None;
    }
    let text = parts.next().unwrap_or("").trim();
    let path = match find_path_start(text) {
        Some(i) => &text[i..],
        None => text,
    };
    Some((code, path.to_string()))
}

/// Posición donde empieza una ruta `X:\` o `\\servidor` dentro de un texto.
fn find_path_start(text: &str) -> Option<usize> {
    let b = text.as_bytes();
    (0..b.len()).find(|&i| {
        let at_word_start = i == 0 || b[i - 1] == b' ';
        let is_unc = b[i..].starts_with(b"\\\\");
        let is_drive =
            i + 2 < b.len() && b[i].is_ascii_alphabetic() && b[i + 1] == b':' && b[i + 2] == b'\\';
        at_word_start && (is_unc || is_drive)
    })
}

/// Último componente de una ruta; vacío si es una carpeta (termina en `\`).
fn base_name(path: &str) -> &str {
    path.rsplit('\\').next().unwrap_or(path)
}

/// Decodifica salida de robocopy, que escribe en la página OEM del sistema
/// (CP850 en Windows en español), no en Windows-1252.
pub fn decode_oem(bytes: &[u8]) -> String {
    use windows_sys::Win32::Globalization::{MultiByteToWideChar, CP_OEMCP};

    if bytes.is_empty() {
        return String::new();
    }
    let len = bytes.len() as i32;
    let needed =
        unsafe { MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), len, std::ptr::null_mut(), 0) };
    let mut wide = vec![0u16; needed.max(0) as usize];
    let written =
        unsafe { MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), len, wide.as_mut_ptr(), needed) };
    String::from_utf16_lossy(&wide[..written.max(0) as usize])
}

/// Publica el fallo pendiente, si lo hay.
fn commit_failure(
    pending: &mut Option<FailedFile>,
    failed_files: &mut Vec<FailedFile>,
    on_copy_error: &mut impl FnMut(CopyErrorEvent),
) {
    if let Some(file) = pending.take() {
        on_copy_error(CopyErrorEvent {
            file_name: file.file_name.clone(),
            error_code: file.error_code,
        });
        failed_files.push(file);
    }
}

/// Extrae el nombre del archivo de un segmento de "info de archivo".
/// Formato: `Nuevo arch\t\t  80.0 m\tbig.bin` → `big.bin`.
/// Devuelve `None` para cabeceras de directorio (la ruta termina en `\`).
pub fn parse_file_info(segment: &str) -> Option<String> {
    if !segment.contains('\t') {
        return None;
    }
    let last = segment.rsplit('\t').next()?.trim();
    if last.is_empty() || last.ends_with('\\') || last.ends_with('/') {
        return None;
    }
    Some(last.to_string())
}

/// Intenta parsear una fila del resumen final de robocopy.
/// El formato real es locale-dependiente y tiene una etiqueta con dos puntos:
///   ` Archivos:        12        12         0         0         0         0`
///   `Director.:         5         4         1         0         0         0`
/// Robocopy imprime siempre en orden: Directorios, Archivos, Bytes, Tiempo.
/// Con `/BYTES` la fila de Bytes también son enteros (sin unidad), así que
/// también parsea. La fila de Tiempo ("0:00:00") no parsea como u64 →
/// descartada automáticamente. Las filas de archivo individual no tienen dos
/// puntos → también descartadas. Se exige 6 enteros tras `:`.
/// Retorna (total, copied, skipped, mismatch, failed, extras).
pub fn parse_summary_line(line: &str) -> Option<(u64, u64, u64, u64, u64, u64)> {
    let colon = line.find(':')?;
    let rest = &line[colon + 1..];
    let nums: Vec<u64> = rest
        .split_whitespace()
        .filter_map(|t| t.parse::<u64>().ok())
        .collect();
    if nums.len() < 6 {
        return None;
    }
    Some((nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]))
}

/// Contadores acumulados durante un escaneo (`/L`).
#[derive(Default)]
pub struct ScanCounters {
    pub file_count: u64,
    pub extras_count: u64,
    pub total_bytes: u64,
    summary_rows_seen: u32,
}

impl ScanCounters {
    /// Procesa una línea de salida de robocopy y actualiza los contadores.
    ///
    /// Las filas del resumen llegan en orden: Directorios (1), Archivos (2),
    /// Bytes (3), Tiempo (descartada). Con `/BYTES`, la fila de Bytes es
    /// entera y su columna "Copiado" (`nums[1]`) es el total de bytes a copiar.
    pub fn process_line(&mut self, line: &str) {
        if let Some((_, copiados, _, _, _, extras)) = parse_summary_line(line) {
            self.summary_rows_seen += 1;
            match self.summary_rows_seen {
                2 => {
                    self.file_count = copiados;
                    self.extras_count = extras;
                }
                3 => {
                    self.total_bytes = copiados;
                }
                _ => {}
            }
        }
    }
}

/// Núcleo de la copia real, desacoplado de Tauri para poder testearlo.
///
/// Lanza robocopy, drena su stdout línea a línea y emite callbacks por cada
/// archivo completado o error. El `Child` se guarda en `child_slot` para que
/// `cancel_core` pueda matarlo desde otro comando. Devuelve el resultado final.
pub async fn run_robocopy_core(
    params: &RobocopyParams,
    total_files: u64,
    child_slot: &SharedChild,
    mut on_file_completed: impl FnMut(FileCompletedEvent),
    mut on_file_progress: impl FnMut(FileProgressEvent),
    mut on_copy_error: impl FnMut(CopyErrorEvent),
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
        let mut guard = child_slot.lock().map_err(|e| e.to_string())?;
        *guard = Some(child);
    }

    let start = Instant::now();
    let mut remaining = start_total;
    let mut copied_summary = 0u64;
    let mut skipped = 0u64;
    let mut failed_dirs_summary = 0u64;
    let mut failed_files_summary = 0u64;
    let mut summary_rows_seen = 0u32;
    let mut failed_files: Vec<FailedFile> = Vec::new();
    let mut current_file: Option<String> = None;
    // Un error no es definitivo hasta saber si el reintento (/R:1) funcionó.
    let mut pending: Option<FailedFile> = None;

    let mut buf: Vec<u8> = Vec::new();
    loop {
        buf.clear();
        // Leer por '\r' para capturar el progreso por archivo en vivo
        // (robocopy separa cada avance con carriage return).
        let n = reader
            .read_until(b'\r', &mut buf)
            .await
            .map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        let seg = decode_oem(&buf);
        let seg = seg.trim();
        if seg.is_empty() {
            continue;
        }

        // Porcentaje: "1.2%", "57%", "100%".
        if let Some(pct) = parse_percent(seg) {
            if pct >= 100 {
                let name = current_file.take().unwrap_or_default();
                // El reintento terminó bien: el error anterior no fue definitivo.
                if pending
                    .as_ref()
                    .is_some_and(|p| base_name(&p.file_name) == name)
                {
                    pending = None;
                }
                remaining = remaining.saturating_sub(1);
                on_file_completed(FileCompletedEvent {
                    remaining,
                    file_name: name,
                });
            } else {
                on_file_progress(FileProgressEvent {
                    file_name: current_file.clone().unwrap_or_default(),
                    percent: pct,
                });
            }
            continue;
        }

        // Línea de error real ("2026/10/30 15:01:09 ERROR 5 (0x00000005) ...").
        // Un reintento del mismo archivo reemplaza al pendiente, no lo duplica.
        if let Some((error_code, file_name)) = parse_error_line(seg) {
            if pending.as_ref().is_some_and(|p| p.file_name != file_name) {
                commit_failure(&mut pending, &mut failed_files, &mut on_copy_error);
            }
            pending = Some(FailedFile {
                file_name,
                error_code,
                message: String::new(),
            });
            continue;
        }

        // Filas del resumen con datos (" Archivos: 3 3 0 0 0 0"): la primera es
        // "Directorios" y la segunda "Archivos".
        if let Some((_, copiados, omitidos, _, fallidos, _)) = parse_summary_line(seg) {
            summary_rows_seen += 1;
            match summary_rows_seen {
                1 => failed_dirs_summary = fallidos,
                2 => {
                    copied_summary = copiados;
                    skipped = omitidos;
                    failed_files_summary = fallidos;
                }
                _ => {}
            }
            continue;
        }

        // Info de archivo ("Nuevo arch\t  80.0 m\tbig.bin"). Si es otro archivo,
        // robocopy ya abandonó el pendiente.
        if let Some(name) = parse_file_info(seg) {
            if pending
                .as_ref()
                .is_some_and(|p| base_name(&p.file_name) != name)
            {
                commit_failure(&mut pending, &mut failed_files, &mut on_copy_error);
            }
            current_file = Some(name);
            continue;
        }

        // La línea siguiente a un error es su descripción ("Acceso denegado.").
        if let Some(p) = pending.as_mut().filter(|p| p.message.is_empty()) {
            p.message = seg.to_string();
        }
    }
    commit_failure(&mut pending, &mut failed_files, &mut on_copy_error);

    // Reobtener el child guardado para esperar su exit code.
    let mut taken: Option<Child> = {
        let mut guard = child_slot.lock().map_err(|e| e.to_string())?;
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

    // El conteo autoritativo es el del resumen: columna "ERROR" de carpetas más
    // archivos, y "Copiado" de archivos. Si robocopy terminó sin resumen (origen
    // inválido, cancelación), caemos a lo detectado durante la copia.
    let (copied, failed) = if summary_rows_seen >= 2 {
        (copied_summary, failed_dirs_summary + failed_files_summary)
    } else {
        (
            start_total.saturating_sub(remaining),
            failed_files.len() as u64,
        )
    };
    let result = RobocopyResult {
        status,
        copied,
        skipped,
        failed,
        failed_files,
        duration_secs,
    };

    Ok(result)
}

/// Núcleo de la cancelación, desacoplado de Tauri.
///
/// Mata el proceso hijo guardado en `child_slot` (si lo hay) y lo "reapa"
/// con `wait()` para liberar recursos.
pub async fn cancel_core(child_slot: &SharedChild) -> Result<(), String> {
    let mut taken: Option<Child> = {
        let mut guard = child_slot.lock().map_err(|e| e.to_string())?;
        guard.take()
    };
    if let Some(child) = taken.as_mut() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn robocopy_status_serializes_pascal_case() {
        assert_eq!(
            serde_json::to_value(RobocopyStatus::Success).unwrap(),
            serde_json::json!("Success")
        );
        assert_eq!(
            serde_json::to_value(RobocopyStatus::NothingToDo).unwrap(),
            serde_json::json!("NothingToDo")
        );
        assert_eq!(
            serde_json::to_value(RobocopyStatus::SomeFailed).unwrap(),
            serde_json::json!("SomeFailed")
        );
        assert_eq!(
            serde_json::to_value(RobocopyStatus::Unknown(8)).unwrap(),
            serde_json::json!({ "Unknown": 8 })
        );
    }

    #[test]
    fn parse_error_line_real_format() {
        assert_eq!(
            parse_error_line(
                "2026/10/30 15:01:09 ERROR 5 (0x00000005) Obteniendo acceso al directorio de origen C:\\System Volume Information\\"
            ),
            Some((5, "C:\\System Volume Information\\".to_string()))
        );
        assert_eq!(
            parse_error_line(
                "2026/10/30 15:01:09 ERROR 32 (0x00000020) Copying File \\\\srv\\share\\mi archivo.txt"
            ),
            Some((32, "\\\\srv\\share\\mi archivo.txt".to_string()))
        );
    }

    #[test]
    fn parse_error_line_ignores_false_positives() {
        assert_eq!(
            parse_error_line("Nuevo dir  3\tD:\\node_modules\\http-errors\\"),
            None
        );
        assert_eq!(parse_error_line("D:\\logs\\FAILED\\"), None);
        assert_eq!(
            parse_error_line("Total   Copiado   Omitido  No coincidencia  ERROR  Extras"),
            None
        );
        assert_eq!(
            parse_error_line("ERROR : parámetro no válido #3 : \"L:/\""),
            None
        );
    }

    #[test]
    fn map_exit_code_handles_bitmask() {
        let name = |c| serde_json::to_value(map_exit_code(c)).unwrap();
        assert_eq!(name(0), serde_json::json!("NothingToDo"));
        assert_eq!(name(1), serde_json::json!("Success"));
        assert_eq!(name(2), serde_json::json!("ExtraFiles"));
        assert_eq!(name(3), serde_json::json!("SuccessWithExtra"));
        for code in [4, 5, 6, 7] {
            assert_eq!(name(code), serde_json::json!("Mismatch"), "exit {code}");
        }
        for code in 8..16 {
            assert_eq!(name(code), serde_json::json!("SomeFailed"), "exit {code}");
        }
        assert_eq!(name(16), serde_json::json!("FatalError"));
        assert_eq!(name(24), serde_json::json!("FatalError"));
        assert_eq!(name(-1), serde_json::json!({ "Unknown": -1 }));
    }

    #[test]
    fn parse_summary_line_spanish() {
        assert_eq!(
            parse_summary_line(
                " Archivos:         3         3         0         0         0         0"
            ),
            Some((3, 3, 0, 0, 0, 0))
        );
        assert_eq!(
            parse_summary_line(
                "Director.:         1         0         1         0         0         0"
            ),
            Some((1, 0, 1, 0, 0, 0))
        );
    }

    #[test]
    fn parse_summary_line_skips_tiempo() {
        // La fila de tiempo no parsea como 6 enteros.
        assert_eq!(
            parse_summary_line(
                "   Tiempo:   0:00:00   0:00:00                       0:00:00   0:00:00"
            ),
            None
        );
    }

    #[test]
    fn parse_percent_works() {
        assert_eq!(parse_percent("  57%  "), Some(57));
        assert_eq!(parse_percent("  1.2%  "), Some(1));
        assert_eq!(parse_percent(" 100%  "), Some(100));
        assert_eq!(parse_percent("Nuevo arch  80.0 m  big.bin"), None);
    }

    #[test]
    fn parse_file_info_works() {
        assert_eq!(
            parse_file_info("Nuevo arch\t\t  80.0 m\tbig.bin"),
            Some("big.bin".to_string())
        );
        // Cabecera de directorio (ruta termina en '\') → None.
        assert_eq!(parse_file_info("3\tC:\\Users\\x\\origen\\"), None);
        // Sin tab → None.
        assert_eq!(parse_file_info("Total   Copiado   Omitido"), None);
    }

    #[test]
    fn build_run_args_incremental() {
        let args = build_run_args("C:\\a", "D:\\b", "incremental", &[]);
        assert_eq!(args, vec!["C:\\a", "D:\\b", "/E", "/W:1", "/R:1", "/XJ"]);
    }

    #[test]
    fn build_run_args_mirror() {
        let args = build_run_args("C:\\a", "D:\\b", "mirror", &[]);
        assert_eq!(args, vec!["C:\\a", "D:\\b", "/MIR", "/W:1", "/R:1", "/XJ"]);
    }

    #[test]
    fn build_run_args_exclude() {
        let excluir = vec!["C:\\Temp".to_string(), "D:\\Cache".to_string()];
        let args = build_run_args("C:\\a", "D:\\b", "incremental", &excluir);
        assert_eq!(
            args,
            vec![
                "C:\\a",
                "D:\\b",
                "/E",
                "/W:1",
                "/R:1",
                "/XJ",
                "/XD",
                "C:\\Temp",
                "D:\\Cache"
            ]
        );
    }

    #[test]
    fn build_scan_args_incremental() {
        let args = build_scan_args("C:\\a", "D:\\b", "incremental", &[]);
        assert_eq!(
            args,
            vec![
                "C:\\a", "D:\\b", "/L", "/E", "/W:1", "/R:1", "/XJ", "/NFL", "/NDL", "/NJH",
                "/BYTES"
            ]
        );
    }

    #[test]
    fn build_scan_args_mirror() {
        let args = build_scan_args("C:\\a", "D:\\b", "mirror", &[]);
        assert_eq!(
            args,
            vec![
                "C:\\a", "D:\\b", "/L", "/MIR", "/W:1", "/R:1", "/XJ", "/NFL", "/NDL", "/NJH",
                "/BYTES"
            ]
        );
    }

    #[test]
    fn build_scan_args_exclude() {
        let excluir = vec!["C:\\Temp".to_string()];
        let args = build_scan_args("C:\\a", "D:\\b", "incremental", &excluir);
        assert_eq!(
            args,
            vec![
                "C:\\a", "D:\\b", "/L", "/E", "/W:1", "/R:1", "/XJ", "/NFL", "/NDL", "/NJH",
                "/BYTES", "/XD", "C:\\Temp"
            ]
        );
    }

    #[test]
    fn process_line_captures_total_bytes() {
        let mut c = ScanCounters::default();
        c.process_line(" Director.:         1         0         1         0         0         0");
        c.process_line(" Archivos:         3         3         0         0         0         0");
        c.process_line("    Bytes:    123456    123456         0         0         0         0");
        assert_eq!(c.file_count, 3);
        assert_eq!(c.total_bytes, 123456);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn e2e_incremental_copy_emits_events() {
        let base = std::env::temp_dir().join(format!("plagg_e2e_{}", std::process::id()));
        let origen = base.join("origen");
        let destino = base.join("destino");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&origen).unwrap();
        std::fs::create_dir_all(&destino).unwrap();

        let total = 5u64;
        for i in 0..total {
            std::fs::write(origen.join(format!("file_{i}.txt")), format!("content {i}")).unwrap();
        }

        let params = RobocopyParams {
            origen: origen.to_string_lossy().to_string(),
            destino: destino.to_string_lossy().to_string(),
            modo: "incremental".to_string(),
            excluir: vec![],
        };

        let child_slot = crate::state::shared_child();
        let completed = std::sync::Arc::new(Mutex::new(Vec::<String>::new()));
        let errors = std::sync::Arc::new(Mutex::new(Vec::<String>::new()));

        let c1 = completed.clone();
        let e1 = errors.clone();
        let result = run_robocopy_core(
            &params,
            total,
            &child_slot,
            move |e: FileCompletedEvent| c1.lock().unwrap().push(e.file_name),
            |_e: FileProgressEvent| {},
            move |e: CopyErrorEvent| e1.lock().unwrap().push(e.file_name),
        )
        .await
        .unwrap();

        assert_eq!(completed.lock().unwrap().len() as u64, total);
        assert!(errors.lock().unwrap().is_empty());
        assert_eq!(result.copied, total);
        for i in 0..total {
            assert!(
                destino.join(format!("file_{i}.txt")).exists(),
                "falta file_{i}.txt en destino"
            );
        }

        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn cancel_stops_copy() {
        let base = std::env::temp_dir().join(format!("plagg_cancel_{}", std::process::id()));
        let origen = base.join("origen");
        let destino = base.join("destino");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&origen).unwrap();
        std::fs::create_dir_all(&destino).unwrap();

        let total = 2000u64;
        for i in 0..total {
            std::fs::write(origen.join(format!("file_{i}.txt")), format!("data {i}")).unwrap();
        }

        let params = RobocopyParams {
            origen: origen.to_string_lossy().to_string(),
            destino: destino.to_string_lossy().to_string(),
            modo: "incremental".to_string(),
            excluir: vec![],
        };

        let child_slot = crate::state::shared_child();
        let completed = std::sync::Arc::new(Mutex::new(Vec::<String>::new()));

        let c1 = completed.clone();
        let slot2 = child_slot.clone();
        let params2 = params.clone();
        let handle = tokio::spawn(async move {
            run_robocopy_core(
                &params2,
                total,
                &slot2,
                move |e: FileCompletedEvent| c1.lock().unwrap().push(e.file_name),
                |_e: FileProgressEvent| {},
                |_e: CopyErrorEvent| {},
            )
            .await
        });

        // Esperar a que el child esté en el slot (copia en curso) y cancelar.
        for _ in 0..1000 {
            if child_slot.lock().unwrap().is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }

        cancel_core(&child_slot).await.unwrap();

        let result = handle.await.unwrap();
        assert!(
            result.is_ok(),
            "la copia debería terminar sin error tras cancelar"
        );
        let n = completed.lock().unwrap().len() as u64;
        assert!(
            n < total,
            "la copia debería haberse cancelado antes de completar los {total} archivos (completados: {n})"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn e2e_large_file_emits_progress() {
        let base = std::env::temp_dir().join(format!("plagg_progress_{}", std::process::id()));
        let origen = base.join("origen");
        let destino = base.join("destino");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&origen).unwrap();
        std::fs::create_dir_all(&destino).unwrap();

        // Archivo grande (~20 MB) para que robocopy emita porcentajes intermedios.
        let big = origen.join("big.bin");
        let data = vec![0xABu8; 20 * 1024 * 1024];
        std::fs::write(&big, &data).unwrap();

        let params = RobocopyParams {
            origen: origen.to_string_lossy().to_string(),
            destino: destino.to_string_lossy().to_string(),
            modo: "incremental".to_string(),
            excluir: vec![],
        };

        let child_slot = crate::state::shared_child();
        let progress = std::sync::Arc::new(Mutex::new(Vec::<(String, u32)>::new()));
        let completed = std::sync::Arc::new(Mutex::new(0u64));
        let p1 = progress.clone();
        let c1 = completed.clone();
        let result = run_robocopy_core(
            &params,
            1,
            &child_slot,
            move |_e: FileCompletedEvent| *c1.lock().unwrap() += 1,
            move |e: FileProgressEvent| p1.lock().unwrap().push((e.file_name, e.percent)),
            |_e: CopyErrorEvent| {},
        )
        .await
        .unwrap();

        let progress = progress.lock().unwrap();
        assert!(
            progress.len() > 1,
            "se esperaban porcentajes intermedios, hubo {}",
            progress.len()
        );
        assert_eq!(*completed.lock().unwrap(), 1);
        assert_eq!(result.copied, 1);
        for (name, pct) in progress.iter() {
            assert_eq!(name.as_str(), "big.bin");
            assert!(*pct <= 100);
        }

        let _ = std::fs::remove_dir_all(&base);
    }

    /// Escaneo completo (sin timeout) para tests: devuelve los contadores.
    #[cfg(windows)]
    async fn scan_with(params: &RobocopyParams) -> ScanCounters {
        let args = build_scan_args(
            &params.origen,
            &params.destino,
            &params.modo,
            &params.excluir,
        );
        let mut child = tokio::process::Command::new("robocopy")
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut counters = ScanCounters::default();
        let mut buf: Vec<u8> = Vec::new();
        loop {
            buf.clear();
            let n = tokio::io::AsyncBufReadExt::read_until(&mut reader, b'\n', &mut buf)
                .await
                .unwrap();
            if n == 0 {
                break;
            }
            let line = encoding_rs::WINDOWS_1252.decode(&buf).0;
            counters.process_line(&line);
        }
        let _ = child.wait().await;
        counters
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn e2e_scan_mirror_detects_extras() {
        let base = std::env::temp_dir().join(format!("plagg_scanmirror_{}", std::process::id()));
        let origen = base.join("origen");
        let destino = base.join("destino");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&origen).unwrap();
        std::fs::create_dir_all(&destino).unwrap();

        // 3 archivos en origen; destino con esos 3 + 2 extras.
        for i in 0..3 {
            std::fs::write(origen.join(format!("file_{i}.txt")), format!("orig {i}")).unwrap();
            std::fs::write(destino.join(format!("file_{i}.txt")), format!("dest {i}")).unwrap();
        }
        std::fs::write(destino.join("extra_1.txt"), "extra").unwrap();
        std::fs::write(destino.join("extra_2.txt"), "extra").unwrap();

        let params = RobocopyParams {
            origen: origen.to_string_lossy().to_string(),
            destino: destino.to_string_lossy().to_string(),
            modo: "mirror".to_string(),
            excluir: vec![],
        };

        let counters = scan_with(&params).await;
        assert_eq!(counters.extras_count, 2);
        // Con /BYTES, la fila "Bytes" debe dejar un total de bytes a copiar > 0
        // (los 3 archivos origen difieren de destino, se copiarían).
        assert!(
            counters.total_bytes > 0,
            "total_bytes debería ser > 0 en un scan con archivos a copiar"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn e2e_failed_file_reports_error() {
        use std::os::windows::fs::OpenOptionsExt;

        let base = std::env::temp_dir().join(format!("plagg_fail_{}", std::process::id()));
        let origen = base.join("origen");
        let destino = base.join("destino");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&origen).unwrap();
        std::fs::create_dir_all(&destino).unwrap();

        // 2 archivos copiables + 1 bloqueado (sin compartir → acceso denegado).
        std::fs::write(origen.join("ok_1.txt"), "uno").unwrap();
        std::fs::write(origen.join("ok_2.txt"), "dos").unwrap();
        std::fs::write(origen.join("locked.txt"), "bloqueado").unwrap();

        // Mantener un handle abierto sin permisos de compartir para que robocopy
        // falle al leerlo (ERROR 32, "usado por otro proceso").
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(origen.join("locked.txt"))
            .unwrap();

        let params = RobocopyParams {
            origen: origen.to_string_lossy().to_string(),
            destino: destino.to_string_lossy().to_string(),
            modo: "incremental".to_string(),
            excluir: vec![],
        };

        let child_slot = crate::state::shared_child();
        let errors = std::sync::Arc::new(Mutex::new(Vec::<String>::new()));
        let e1 = errors.clone();
        let result = run_robocopy_core(
            &params,
            3,
            &child_slot,
            |_e: FileCompletedEvent| {},
            |_e: FileProgressEvent| {},
            move |e: CopyErrorEvent| e1.lock().unwrap().push(e.file_name),
        )
        .await
        .unwrap();

        assert_eq!(result.copied, 2, "los dos archivos sanos debían copiarse");
        assert_eq!(
            result.failed, 1,
            "esperaba 1 archivo fallido, hubo {}",
            result.failed
        );
        assert!(
            result
                .failed_files
                .iter()
                .any(|f| f.file_name.ends_with("\\locked.txt") && !f.message.is_empty()),
            "el listado debe contener la ruta de locked.txt con su motivo: {:?}",
            result.failed_files
        );
        assert_eq!(errors.lock().unwrap().len(), 1);

        drop(lock);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Crea origen/destino vacíos en temp y los parámetros de una copia incremental.
    #[cfg(windows)]
    fn temp_copy(tag: &str) -> (std::path::PathBuf, std::path::PathBuf, RobocopyParams) {
        let base = std::env::temp_dir().join(format!("plagg_{tag}_{}", std::process::id()));
        let origen = base.join("origen");
        let destino = base.join("destino");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&origen).unwrap();
        std::fs::create_dir_all(&destino).unwrap();
        let params = RobocopyParams {
            origen: origen.to_string_lossy().to_string(),
            destino: destino.to_string_lossy().to_string(),
            modo: "incremental".to_string(),
            excluir: vec![],
        };
        (base, origen, params)
    }

    /// Copia reuniendo los `copy_error` emitidos.
    #[cfg(windows)]
    async fn run_collecting_errors(
        params: &RobocopyParams,
        total: u64,
    ) -> (RobocopyResult, Vec<String>) {
        let errors = std::sync::Arc::new(Mutex::new(Vec::<String>::new()));
        let e1 = errors.clone();
        let result = run_robocopy_core(
            params,
            total,
            &crate::state::shared_child(),
            |_e: FileCompletedEvent| {},
            |_e: FileProgressEvent| {},
            move |e: CopyErrorEvent| e1.lock().unwrap().push(e.file_name),
        )
        .await
        .unwrap();
        let errors = errors.lock().unwrap().clone();
        (result, errors)
    }

    #[cfg(windows)]
    fn lock_file(path: &std::path::Path) -> std::fs::File {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(path)
            .unwrap()
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn e2e_error_named_folder_is_not_a_failure() {
        let (base, origen, params) = temp_copy("errname");
        for dir in ["http-errors", "FAILED"] {
            std::fs::create_dir_all(origen.join(dir)).unwrap();
            std::fs::write(origen.join(dir).join("error.log"), "ok").unwrap();
        }

        let (result, errors) = run_collecting_errors(&params, 2).await;

        assert!(errors.is_empty(), "falsos positivos: {errors:?}");
        assert_eq!(result.failed, 0);
        assert_eq!(result.copied, 2);
        assert!(result.failed_files.is_empty());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn e2e_retry_success_is_not_a_failure() {
        let (base, origen, params) = temp_copy("retry");
        std::fs::write(origen.join("busy.txt"), "ocupado").unwrap();
        let lock = lock_file(&origen.join("busy.txt"));
        // Se libera antes del reintento (/W:1).
        let release = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(400));
            drop(lock);
        });

        let (result, errors) = run_collecting_errors(&params, 1).await;
        release.join().unwrap();

        assert!(errors.is_empty(), "no debió reportar fallo: {errors:?}");
        assert_eq!(result.failed, 0);
        assert_eq!(result.copied, 1);
        assert!(result.failed_files.is_empty());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn e2e_inaccessible_subfolder_is_reported() {
        let (base, origen, params) = temp_copy("denied");
        std::fs::write(origen.join("ok.txt"), "ok").unwrap();
        let denied = origen.join("privada");
        std::fs::create_dir_all(&denied).unwrap();
        std::fs::write(denied.join("secreto.txt"), "x").unwrap();
        let user = std::env::var("USERNAME").unwrap();
        let icacls = |args: &[&str]| {
            std::process::Command::new("icacls")
                .arg(&denied)
                .args(args)
                .output()
                .unwrap()
        };
        assert!(icacls(&["/deny", &format!("{user}:(RX)")]).status.success());

        let (result, errors) = run_collecting_errors(&params, 2).await;

        icacls(&["/remove:d", &user]);
        assert_eq!(result.failed, 1, "{:?}", result.failed_files);
        assert_eq!(errors.len(), 1);
        assert_eq!(result.failed_files.len(), 1);
        assert!(
            result.failed_files[0].file_name.ends_with("\\privada\\"),
            "{:?}",
            result.failed_files
        );
        assert_eq!(result.failed_files[0].error_code, 5);
        assert!(matches!(result.status, RobocopyStatus::SomeFailed));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn e2e_accented_name_survives_decoding() {
        let (base, origen, params) = temp_copy("accent");
        let path = origen.join("ñandú.txt");
        std::fs::write(&path, "x").unwrap();
        let lock = lock_file(&path);

        let (result, _) = run_collecting_errors(&params, 1).await;

        drop(lock);
        assert_eq!(result.failed_files.len(), 1);
        assert!(
            result.failed_files[0].file_name.ends_with("\\ñandú.txt"),
            "{:?}",
            result.failed_files
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn e2e_same_name_in_two_folders_are_two_failures() {
        let (base, origen, params) = temp_copy("twins");
        let mut locks = Vec::new();
        for dir in ["a", "b"] {
            std::fs::create_dir_all(origen.join(dir)).unwrap();
            let path = origen.join(dir).join("thumbs.db");
            std::fs::write(&path, "x").unwrap();
            locks.push(lock_file(&path));
        }

        let (result, errors) = run_collecting_errors(&params, 2).await;

        drop(locks);
        assert_eq!(result.failed, 2);
        assert_eq!(result.failed_files.len(), 2, "{:?}", result.failed_files);
        assert_eq!(errors.len(), 2);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn e2e_junction_is_skipped() {
        let (base, origen, params) = temp_copy("junction");
        let fuera = base.join("fuera");
        std::fs::create_dir_all(&fuera).unwrap();
        std::fs::write(fuera.join("ajeno.txt"), "x").unwrap();
        std::fs::write(origen.join("propio.txt"), "x").unwrap();
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(origen.join("enlace"))
            .arg(&fuera)
            .output()
            .unwrap()
            .status;
        assert!(status.success());

        let (result, errors) = run_collecting_errors(&params, 1).await;

        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(result.failed, 0);
        assert_eq!(result.copied, 1);
        let destino = std::path::Path::new(&params.destino);
        assert!(destino.join("propio.txt").exists());
        assert!(!destino.join("enlace").exists());

        let _ = std::fs::remove_dir_all(&base);
    }
}
