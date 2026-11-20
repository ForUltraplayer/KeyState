use super::AppConfig;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn config_path() -> PathBuf {
    exe_dir().join("keystate_config.json")
}

pub fn profiles_dir() -> PathBuf {
    exe_dir().join("profiles")
}

pub fn records_dir() -> PathBuf {
    exe_dir().join("records")
}

/// [M-4] 설정 및 프로필 공통 무결성 검증/보정
pub fn sanitize_config(cfg: &mut AppConfig) {
    if cfg.themes.is_empty() {
        cfg.themes = super::presets::default_themes();
    }
    if cfg.active_theme >= cfg.themes.len() {
        cfg.active_theme = 0;
    }
    // IME VK_PROCESSKEY (229) 제거
    cfg.keys.retain(|k| k.vk != 229);

    // 0 또는 중복 ID 완벽 보정
    let mut used_ids = HashSet::new();
    let mut next_id = cfg
        .next_key_id
        .max(cfg.keys.iter().map(|k| k.id).max().unwrap_or(0) + 1)
        .max(1);
    for k in &mut cfg.keys {
        if k.id == 0 || !used_ids.insert(k.id) {
            while !used_ids.insert(next_id) {
                next_id += 1;
            }
            k.id = next_id;
            next_id += 1;
        }
        k.width = k.width.clamp(24.0, 500.0);
        k.height = k.height.clamp(24.0, 300.0);
    }
    cfg.next_key_id = next_id;
    cfg.fps = cfg.fps.clamp(24, 240);
    cfg.overlay_opacity = cfg.overlay_opacity.clamp(0.05, 1.0);
    cfg.bpm_note_div = match cfg.bpm_note_div {
        8 | 16 | 32 | 64 => cfg.bpm_note_div,
        _ => 16,
    };
}

/// [M-4] 임시 파일 작성 후 원자적 교체(rename)로 크래시 시 설정 손상 방지
fn write_file_atomic(path: &Path, content: &str) -> std::io::Result<()> {
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, content)?;
    if fs::rename(&tmp_path, path).is_err() {
        // 일부 환경에서 rename 실패 시 직접 쓰기 폴백
        fs::write(path, content)?;
        let _ = fs::remove_file(&tmp_path);
    }
    Ok(())
}

pub fn load_config() -> AppConfig {
    let path = config_path();
    if let Ok(text) = fs::read_to_string(&path) {
        if let Ok(mut cfg) = serde_json::from_str::<AppConfig>(&text) {
            sanitize_config(&mut cfg);
            return cfg;
        }
    }
    let cfg = AppConfig::default();
    let _ = save_config(&cfg);
    cfg
}

pub fn save_config(cfg: &AppConfig) -> std::io::Result<()> {
    let path = config_path();
    let json = serde_json::to_string_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    write_file_atomic(&path, &json)
}

pub fn list_profiles() -> Vec<String> {
    let dir = profiles_dir();
    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
    }
    names.sort();
    names
}

fn sanitize_profile_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => c,
        })
        .collect();
    if cleaned.is_empty() {
        "profile".to_string()
    } else {
        cleaned
    }
}

pub fn save_profile(name: &str, cfg: &AppConfig) -> std::io::Result<String> {
    let dir = profiles_dir();
    fs::create_dir_all(&dir)?;
    let safe = sanitize_profile_name(name);
    let path = dir.join(format!("{safe}.json"));
    let json = serde_json::to_string_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    write_file_atomic(&path, &json)?;
    Ok(safe)
}

pub fn load_profile(name: &str) -> Option<AppConfig> {
    let safe = sanitize_profile_name(name);
    let path = profiles_dir().join(format!("{safe}.json"));
    let text = fs::read_to_string(path).ok()?;
    let mut cfg: AppConfig = serde_json::from_str(&text).ok()?;
    cfg.active_profile = safe;
    sanitize_config(&mut cfg);
    Some(cfg)
}

pub fn delete_profile(name: &str) -> std::io::Result<()> {
    let safe = sanitize_profile_name(name);
    let path = profiles_dir().join(format!("{safe}.json"));
    if Path::new(&path).exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}
