use std::fs;
use std::path::PathBuf;

const DEFAULT_PRESETS: &[f32] = &[88.6, 96.6, 97.2, 97.6, 98.3, 100.4, 104.4];

fn get_presets_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/fm_presets.json")
    } else {
        PathBuf::from("/home/user/.config/fm_presets.json")
    }
}

pub fn load_presets() -> Vec<f32> {
    let path = get_presets_path();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(mut list) = serde_json::from_str::<Vec<f32>>(&content) {
            list.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            list.dedup_by(|a, b| (*a - *b).abs() < 0.05);
            if !list.is_empty() {
                return list;
            }
        }
    }
    DEFAULT_PRESETS.to_vec()
}

pub fn save_presets(presets: &[f32]) {
    let path = get_presets_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut sorted = presets.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    sorted.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    if let Ok(json) = serde_json::to_string_pretty(&sorted) {
        let _ = fs::write(&path, json);
    }
}
