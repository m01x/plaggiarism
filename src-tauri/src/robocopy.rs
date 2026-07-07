use serde::{Deserialize, Serialize};

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

/// Intenta extraer el nombre del archivo de una línea que comienza con `100%`.
/// Robocopy imprime algo como:
///   `100%  New File   1.23 mb  ruta\archivo.ext`
/// Retorna `Some(nombre)` si la línea es candidata, `None` si no.
pub fn parse_file_completed(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("100%") {
        return None;
    }

    // Saltar varios tokens hasta el nombre del archivo.
    // Formato típico: "100%\t<tipo>\t<tamaño>\t<nombre>"
    let mut tokens = trimmed.split_whitespace();
    tokens.next()?; // "100%"
    tokens.next()?; // tipo (New File | newer | ...)
    tokens.next()?; // tamaño (1.23mb, 512kb, ...)

    let rest: Vec<&str> = tokens.collect();
    if rest.is_empty() {
        return None;
    }
    Some(rest.join(" "))
}

/// Intenta parsear la línea de resumen final de robocopy:
///   `    Total    Copied   Skipped  Mismatch    FAILED    Extras`
///   `     1234       456       778         0         0         0`
/// Retorna (total, copied, skipped, mismatch, failed, extras).
pub fn parse_summary_line(line: &str) -> Option<(u64, u64, u64, u64, u64, u64)> {
    let trimmed = line.trim();
    // Debe ser solo dígitos y espacios.
    if trimmed.is_empty()
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_digit() || c.is_whitespace())
    {
        return None;
    }
    let nums: Vec<u64> = trimmed
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
