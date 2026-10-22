//! Photo storage.
//!
//! Files live in a flat directory so the frontend can address them with
//! `/uploads/{filename}`. Names are generated, never taken from the client,
//! and an allowlist keeps non-image uploads out.

use std::path::{Path, PathBuf};

use uuid::Uuid;

/// Where uploads live, relative to the working directory unless overridden.
pub fn upload_dir() -> PathBuf {
    std::env::var("PADDOCK_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./data"))
        .join("uploads")
}

pub fn ensure_upload_dir() -> std::io::Result<()> {
    std::fs::create_dir_all(upload_dir())
}

/// Image types accepted for attachment. HEIC is included because phone cameras
/// produce it by default.
fn is_allowed_mime(mime: &str) -> bool {
    matches!(
        mime.to_lowercase().as_str(),
        "image/jpeg"
            | "image/jpg"
            | "image/png"
            | "image/webp"
            | "image/gif"
            | "image/heic"
            | "image/heif"
    )
}

/// Derive a stored extension from the client's filename, capped in length.
///
/// Falls back to `.jpg` when the original has no usable extension.
pub fn safe_extension(original: &str) -> String {
    let ext = Path::new(original)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.trim_start_matches('.').to_lowercase())
        .filter(|e| !e.is_empty() && e.len() <= 8 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or_else(|| "jpg".to_string());
    format!(".{ext}")
}

/// Persist one uploaded photo, returning `(filename, original, mime, size)`.
///
/// Returns an error string when the type is not an allowed image or the write
/// fails, so the handler can surface the reason.
pub fn store(
    original_name: &str,
    mime: &str,
    bytes: &[u8],
) -> Result<(String, String, String, i64), String> {
    if !is_allowed_mime(mime) {
        return Err("Only image files are supported".into());
    }

    let dir = upload_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let filename = format!("{}{}", Uuid::new_v4(), safe_extension(original_name));
    let path = dir.join(&filename);
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;

    Ok((
        filename,
        original_name.to_string(),
        mime.to_string(),
        bytes.len() as i64,
    ))
}

/// Remove a stored photo. Missing files are not an error.
pub fn remove_file(filename: &str) {
    if filename.is_empty() {
        return;
    }
    let path = upload_dir().join(filename);
    let _ = std::fs::remove_file(path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_images_are_accepted() {
        assert!(is_allowed_mime("image/png"));
        assert!(is_allowed_mime("IMAGE/JPEG"));
        assert!(is_allowed_mime("image/heic"));
        assert!(!is_allowed_mime("application/pdf"));
        assert!(!is_allowed_mime("text/plain"));
        assert!(!is_allowed_mime(""));
    }

    #[test]
    fn extensions_are_sanitised() {
        assert_eq!(safe_extension("photo.JPG"), ".jpg");
        assert_eq!(safe_extension("photo.heic"), ".heic");
        assert_eq!(safe_extension("noextension"), ".jpg");
        assert_eq!(safe_extension("weird.averylongext"), ".jpg");
        assert_eq!(safe_extension("bad.na/me"), ".jpg", "paths cannot escape");
    }
}
