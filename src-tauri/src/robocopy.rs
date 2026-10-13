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
    SomeFailed,
    FatalError,
    Unknown(i32),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RobocopyResult {
    pub status: RobocopyStatus,
    pub copied: u64,
    pub skipped: u64,
    pub failed: u64,
    pub failed_files: Vec<String>,
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

pub fn map_exit_code(code: i32) -> RobocopyStatus {
    match code {
        0 => RobocopyStatus::NothingToDo,
        1 => RobocopyStatus::Success,
        2 => RobocopyStatus::ExtraFiles,
        3 => RobocopyStatus::SuccessWithExtra,
        8 => RobocopyStatus::SomeFailed,
        16 => RobocopyStatus::FatalError,
        other => RobocopyStatus::Unknown(other),
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

    args.push("/NFL".to_string());
    args.push("/NDL".to_string());
    args.push("/NJH".to_string());

    if !excluir.is_empty() {
        args.push("/XD".to_string());
        for p in excluir {
            args.push(p.clone());
        }
    }

    args
}

/// Extrae el nombre del archivo de una línea que indica copia completada.
///
/// Robocopy (locale ES) emite cada archivo copiado así, usando `\r` para
/// sobrescribir el progreso en consola y `\n` sólo al final de la línea:
///
/// ```text
/// \t    Nuevo arch\t\t      13\tarchivo1.txt\r100%  \r\n
/// ```
///
/// Por tanto, al leer línea a línea (`read_until(b'\n')`) el nombre del archivo
/// es el último campo antes del primer `\r`, y `100%` aparece *después* de él.
/// Sólo los archivos realmente copiados incluyen `100%`; los omitidos no.
pub fn parse_file_completed(line: &str) -> Option<String> {
    if !line.contains("100%") {
        return None;
    }
    // Parte anterior al primer \r: "\t    Nuevo arch\t\t      13\tarchivo1.txt"
    let before = line.split('\r').next()?;
    // El nombre es el último campo separado por tabulador (admite espacios).
    let name = before.rsplit('\t').next()?.trim();
    if name.is_empty() {
        return None;
    }
    Some(name.to_string())
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
/// Las filas de Bytes ("19.1 k") y Tiempo ("0:00:00") no parsean como u64 →
/// son descartadas automáticamente. Las filas de archivo individual no
/// tienen dos puntos → también descartadas. Se exige 6 enteros tras `:`.
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

/// Intenta extraer bytes de una línea con tamaño estilo `1.23 mb`, `512 kb`, `2.5 gb`.
pub fn parse_size_bytes(line: &str) -> Option<u64> {
    let lower = line.to_lowercase();
    let tokens: Vec<&str> = lower.split_whitespace().collect();
    for (i, t) in tokens.iter().enumerate() {
        let unit = *t;
        if let Some(prev) = i.checked_sub(1).and_then(|j| tokens.get(j)) {
            let value: f64 = prev.trim_end_matches(',').parse().ok()?;
            let bytes = match unit {
                "b" | "bytes" => value,
                "kb" => value * 1024.0,
                "mb" => value * 1024.0 * 1024.0,
                "gb" => value * 1024.0 * 1024.0 * 1024.0,
                _ => continue,
            };
            return Some(bytes as u64);
        }
    }
    None
}

/// Contadores acumulados durante un escaneo (`/L`) o una copia.
#[derive(Default)]
pub struct ScanCounters {
    pub file_count: u64,
    pub extras_count: u64,
    pub total_bytes: u64,
    summary_rows_seen: u32,
}

impl ScanCounters {
    /// Procesa una línea de salida de robocopy y actualiza los contadores.
    pub fn process_line(&mut self, line: &str) {
        if let Some((_, copiados, _, _, _, extras)) = parse_summary_line(line) {
            self.summary_rows_seen += 1;
            if self.summary_rows_seen == 2 {
                self.file_count = copiados;
                self.extras_count = extras;
            }
        } else if let Some(name) = parse_file_completed(line) {
            self.file_count = self.file_count.saturating_add(1);
            if let Some(b) = parse_size_bytes(line) {
                self.total_bytes = self.total_bytes.saturating_add(b);
            }
            let _ = name;
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
    let mut skipped = 0u64;
    let mut summary_rows_seen = 0u32;
    let mut failed_files: Vec<String> = Vec::new();
    let mut last_file_for_error: Option<String> = None;
    let mut current_file: Option<String> = None;

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
        let seg = encoding_rs::WINDOWS_1252.decode(&buf).0;
        let seg = seg.trim();
        if seg.is_empty() {
            continue;
        }

        // Porcentaje: "1.2%", "57%", "100%".
        if let Some(pct) = parse_percent(seg) {
            if pct >= 100 {
                let name = current_file.take().unwrap_or_default();
                remaining = remaining.saturating_sub(1);
                last_file_for_error = Some(name.clone());
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

        // Fila del resumen con datos (" Archivos: 3 3 0 0 0 0").
        // La segunda fila es "Archivos": columna 3 (Omitido) = skipped.
        if let Some((_, _, omitidos, _, _, _)) = parse_summary_line(seg) {
            summary_rows_seen += 1;
            if summary_rows_seen == 2 {
                skipped = omitidos;
            }
            continue;
        }

        // Cabecera del resumen ("Total ... ERROR ... Extras") — contiene la
        // palabra "ERROR" como título de columna; saltarla antes del error.
        if seg.contains("Total") {
            continue;
        }

        // Info de archivo ("Nuevo arch\t  80.0 m\tbig.bin").
        if let Some(name) = parse_file_info(seg) {
            current_file = Some(name);
            continue;
        }

        // Línea de error real ("ERROR 2 (0x00000002) ...").
        let up = seg.to_uppercase();
        if up.contains("ERROR") || up.contains("FAILED") {
            let name = last_file_for_error
                .clone()
                .unwrap_or_else(|| seg.to_string());
            failed_files.push(name.clone());
            on_copy_error(CopyErrorEvent {
                file_name: name,
                error_code: 8,
            });
        }
    }

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

    let failed_count = failed_files.len() as u64;
    let result = RobocopyResult {
        status,
        copied: start_total.saturating_sub(remaining),
        skipped,
        failed: failed_count,
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
    fn parse_file_completed_real_format() {
        // Línea real decodificada (WINDOWS_1252), con \r y \n literales.
        let line = "\t    Nuevo arch\t\t      13\tarchivo1.txt\r100%  \r\n";
        assert_eq!(parse_file_completed(line), Some("archivo1.txt".to_string()));
    }

    #[test]
    fn parse_file_completed_ignores_non_copied() {
        // Una línea de archivo omitido no lleva "100%".
        let line = "\t    Mismo\t\t       0\tarchivo_ya_existente.txt\r\n";
        assert_eq!(parse_file_completed(line), None);
    }

    #[test]
    fn parse_file_completed_ignores_summary() {
        let line = " Archivos:         3         3         0         0         0         0";
        assert_eq!(parse_file_completed(line), None);
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
        assert_eq!(args, vec!["C:\\a", "D:\\b", "/E", "/W:1", "/R:1"]);
    }

    #[test]
    fn build_run_args_mirror() {
        let args = build_run_args("C:\\a", "D:\\b", "mirror", &[]);
        assert_eq!(args, vec!["C:\\a", "D:\\b", "/MIR", "/W:1", "/R:1"]);
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
            vec!["C:\\a", "D:\\b", "/L", "/E", "/NFL", "/NDL", "/NJH"]
        );
    }

    #[test]
    fn build_scan_args_mirror() {
        let args = build_scan_args("C:\\a", "D:\\b", "mirror", &[]);
        assert_eq!(
            args,
            vec!["C:\\a", "D:\\b", "/L", "/MIR", "/NFL", "/NDL", "/NJH"]
        );
    }

    #[test]
    fn build_scan_args_exclude() {
        let excluir = vec!["C:\\Temp".to_string()];
        let args = build_scan_args("C:\\a", "D:\\b", "incremental", &excluir);
        assert_eq!(
            args,
            vec!["C:\\a", "D:\\b", "/L", "/E", "/NFL", "/NDL", "/NJH", "/XD", "C:\\Temp"]
        );
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

        let _ = std::fs::remove_dir_all(&base);
    }
}
