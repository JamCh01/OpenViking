// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Directory zip archiving, image data-URI conversion, and atomic file persistence.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use serde_json::Value;
use tempfile::NamedTempFile;
use uuid::Uuid;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::error::{ErrorKind, OpenVikingError, Result};
use crate::options::ImageInput;

/// Compresses a local directory into a temporary `.zip` archive using forward-slash entry names.
///
/// Symbolic links and resolved files outside `dir_path` are skipped. If an error occurs while
/// building the archive, any partial `.zip` file is removed before returning.
pub async fn zip_directory(dir_path: &Path) -> Result<PathBuf> {
    zip_directory_in(dir_path, None).await
}

/// Compresses `dir_path` into a temporary `.zip` archive inside `temp_dir` (or `std::env::temp_dir()`).
pub async fn zip_directory_in(dir_path: &Path, temp_dir: Option<&Path>) -> Result<PathBuf> {
    let source = dir_path.to_path_buf();
    let archive_root = temp_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(std::env::temp_dir);

    tokio::task::spawn_blocking(move || zip_directory_sync(&source, &archive_root))
        .await
        .map_err(|join_err| {
            OpenVikingError::new(
                ErrorKind::Internal,
                "INTERNAL",
                format!("Directory zip worker failed: {join_err}"),
            )
        })?
}

fn zip_directory_sync(dir_path: &Path, archive_dir: &Path) -> Result<PathBuf> {
    if !dir_path.is_dir() {
        return Err(OpenVikingError::invalid_argument(format!(
            "Path {} is not a directory",
            dir_path.display()
        )));
    }

    let canonical_root = dir_path.canonicalize()?;
    let zip_path = archive_dir.join(format!("temp_upload_{}.zip", Uuid::new_v4().simple()));

    let write_result = (|| -> Result<()> {
        let zip_file = File::create(&zip_path)?;
        let mut writer = ZipWriter::new(zip_file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        let mut buffer = Vec::new();

        for entry_res in WalkDir::new(dir_path).follow_links(false) {
            let entry = entry_res.map_err(|err| {
                let io_err = err
                    .into_io_error()
                    .unwrap_or_else(|| std::io::Error::other("walkdir traversal error"));
                OpenVikingError::from(io_err)
            })?;
            if entry.path_is_symlink() || !entry.file_type().is_file() {
                continue;
            }

            let file_path = entry.path();
            let Ok(canonical_file) = file_path.canonicalize() else {
                continue;
            };
            if !canonical_file.starts_with(&canonical_root) {
                continue;
            }

            let relative = file_path.strip_prefix(dir_path).map_err(|err| {
                OpenVikingError::invalid_argument(format!("Failed to compute relative path: {err}"))
            })?;
            let arcname = relative.to_string_lossy().replace('\\', "/");
            if arcname.is_empty() {
                continue;
            }

            writer.start_file(arcname, options).map_err(|err| {
                OpenVikingError::new(ErrorKind::Io, "ZIP_ERROR", err.to_string()).with_source(err)
            })?;
            buffer.clear();
            let mut src = File::open(file_path)?;
            src.read_to_end(&mut buffer)?;
            writer.write_all(&buffer)?;
        }

        let mut finished = writer.finish().map_err(|err| {
            OpenVikingError::new(ErrorKind::Io, "ZIP_ERROR", err.to_string()).with_source(err)
        })?;
        finished.flush()?;
        Ok(())
    })();

    if let Err(err) = write_result {
        let _ = std::fs::remove_file(&zip_path);
        return Err(err);
    }

    Ok(zip_path)
}

/// Atomically writes `data` to `dest` via a sibling temporary file and rename, preserving any
/// existing file at `dest` if writing or renaming fails.
pub async fn atomic_write_bytes(dest: &Path, data: &[u8]) -> Result<()> {
    let target = dest.to_path_buf();
    let payload = data.to_vec();
    tokio::task::spawn_blocking(move || atomic_write_bytes_sync(&target, &payload))
        .await
        .map_err(|join_err| {
            OpenVikingError::new(
                ErrorKind::Internal,
                "INTERNAL",
                format!("Atomic file write worker failed: {join_err}"),
            )
        })?
}

fn atomic_write_bytes_sync(dest: &Path, data: &[u8]) -> Result<()> {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let file_name = dest
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".to_owned());

    let mut temp = tempfile::Builder::new()
        .prefix(&format!(".{file_name}-"))
        .suffix(".tmp")
        .tempfile_in(parent)?;
    temp.write_all(data)?;
    temp.flush()?;
    temp.persist(dest)
        .map_err(|err| OpenVikingError::from(err.error))?;
    Ok(())
}

/// Resolves the output path for an `.ovpack` export or backup, matching Python/Go/TS SDK rules.
pub fn resolve_ovpack_output_path(to: &str, uri: Option<&str>, fallback: &str) -> PathBuf {
    let base_target = if to.is_empty() { "." } else { to };
    let target_path = Path::new(base_target);
    if target_path.is_dir() {
        let stem = uri
            .map(|u| u.trim().trim_end_matches('/'))
            .and_then(|u| u.rsplit('/').next())
            .filter(|name| !name.is_empty() && *name != "viking:")
            .unwrap_or(fallback);
        return target_path.join(format!("{stem}.ovpack"));
    }
    if base_target.ends_with(".ovpack") {
        target_path.to_path_buf()
    } else {
        PathBuf::from(format!("{base_target}.ovpack"))
    }
}

/// Normalizes an [`ImageInput`] into the wire `image_url` JSON value accepted by OpenViking search routes.
pub async fn normalize_image_input(input: &ImageInput) -> Result<Value> {
    match input {
        ImageInput::Bytes { data, file_name } => Ok(Value::String(bytes_to_data_uri(
            data,
            file_name.as_deref().unwrap_or(""),
        ))),
        ImageInput::Reference(raw) => {
            if raw.starts_with("data:image/")
                || raw.starts_with("http://")
                || raw.starts_with("https://")
                || raw.starts_with("viking://")
            {
                return Ok(Value::String(raw.clone()));
            }
            let expanded = expand_user_path(raw);
            if let Ok(meta) = tokio::fs::metadata(&expanded).await {
                if meta.is_file() {
                    let bytes = tokio::fs::read(&expanded).await?;
                    let name = expanded
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    return Ok(Value::String(bytes_to_data_uri(&bytes, &name)));
                }
            }
            Ok(Value::String(raw.clone()))
        }
        ImageInput::Multiple(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(Box::pin(normalize_image_input(item)).await?);
            }
            Ok(Value::Array(out))
        }
    }
}

/// Checks whether a local filesystem `stat` error indicates an overlong inline string (`ENAMETOOLONG`),
/// allowing `add_skill` / `update_skill` to treat long Markdown strings as inline content.
pub fn is_inline_content_path_error(err: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        if err.raw_os_error() == Some(36) || err.raw_os_error() == Some(63) {
            return true;
        }
    }
    #[cfg(windows)]
    {
        if matches!(err.raw_os_error(), Some(123) | Some(206)) {
            return true;
        }
    }
    err.to_string()
        .to_lowercase()
        .contains("file name too long")
}

/// Expands a leading `~/` against `HOME` or `USERPROFILE`.
pub fn expand_user_path(raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(raw)
}

fn bytes_to_data_uri(data: &[u8], file_name: &str) -> String {
    let mime = mime_guess::from_path(file_name)
        .first_raw()
        .filter(|m| m.starts_with("image/"))
        .unwrap_or("image/png");
    let encoded = BASE64_STANDARD.encode(data);
    format!("data:{mime};base64,{encoded}")
}

#[allow(dead_code)]
fn _keep_named_temp_file_used(_: Option<NamedTempFile>) {}
