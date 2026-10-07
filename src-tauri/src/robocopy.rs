use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use crate::state::SharedChild;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
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

pub fn build_scan_args(origen: &str, destino: &str) -> Vec<String> {
    vec![
        origen.to_string(),
        destino.to_string(),
        "/L".to_string(),
        "/E".to_string(),
        "/NFL".to_string(),
        "/NDL".to_string(),
        "/NJH".to_string(),
    ]
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
    let mut failed_files: Vec<String> = Vec::new();
    let mut last_file_for_error: Option<String> = None;

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

        // Fila del resumen con datos (" Archivos: 3 3 0 0 0 0").
        if parse_summary_line(&line).is_some() {
            continue;
        }

        // Archivo completado ("...\tarchivo.txt\r100%  ").
        if let Some(name) = parse_file_completed(&line) {
            remaining = remaining.saturating_sub(1);
            last_file_for_error = Some(name.clone());
            on_file_completed(FileCompletedEvent {
                remaining,
                file_name: name,
            });
            continue;
        }

        // Cabecera del resumen ("Total ... ERROR ... Extras") — contiene la
        // palabra "ERROR" como título de columna, hay que saltarla para no
        // generar un copy_error falso.
        if line.contains("Total") {
            continue;
        }

        // Línea de error real ("ERROR 2 (0x00000002) ...").
        let up = line.to_uppercase();
        if up.contains("ERROR") || up.contains("FAILED") {
            let name = last_file_for_error
                .clone()
                .unwrap_or_else(|| line.trim().to_string());
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
        copied: start_total
            .saturating_sub(remaining)
            .saturating_sub(failed_count),
        skipped: 0,
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
    fn build_scan_args_has_list_only() {
        let args = build_scan_args("C:\\a", "D:\\b");
        assert!(args.contains(&"/L".to_string()));
        assert!(args.contains(&"/NFL".to_string()));
        assert!(args.contains(&"/NDL".to_string()));
        assert!(args.contains(&"/NJH".to_string()));
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
}
