// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! URI normalization utilities for the OpenViking `viking://` virtual filesystem.

/// Helpers for normalizing OpenViking `viking://` URIs and read-only file references.
#[derive(Debug, Clone, Copy, Default)]
pub struct VikingUri;

impl VikingUri {
    /// Normalizes a short or slash-prefixed path into canonical `viking://` form.
    ///
    /// Empty strings are returned unchanged so optional target filters preserve server defaults.
    pub fn normalize(uri: &str) -> String {
        if uri.is_empty() {
            return String::new();
        }
        if uri.starts_with("viking://") {
            return uri.to_owned();
        }
        if uri == "/" {
            return "viking://".to_owned();
        }
        let cleaned = uri.trim();
        let without_leading = cleaned.strip_prefix('/').unwrap_or(cleaned);
        format!("viking://{without_leading}")
    }

    /// Normalizes a URI while preserving 32-character lowercase hex record IDs for read/stat APIs.
    pub fn normalize_file_reference(uri_or_id: &str) -> String {
        let cleaned = uri_or_id.trim();
        let is_record_id = cleaned.len() == 32
            && cleaned
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if is_record_id {
            return cleaned.to_owned();
        }
        Self::normalize(uri_or_id)
    }
}

/// Normalizes a short OpenViking URI into canonical `viking://` form.
pub fn normalize_uri(uri: &str) -> String {
    VikingUri::normalize(uri)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_short_and_root_uris() {
        assert_eq!(VikingUri::normalize(""), "");
        assert_eq!(VikingUri::normalize("/"), "viking://");
        assert_eq!(
            VikingUri::normalize("/resources/demo"),
            "viking://resources/demo"
        );
        assert_eq!(
            VikingUri::normalize("resources/demo"),
            "viking://resources/demo"
        );
        assert_eq!(
            VikingUri::normalize("viking://~/memories"),
            "viking://~/memories"
        );
    }

    #[test]
    fn preserves_32_hex_record_ids_for_file_references() {
        let id = "0123456789abcdef0123456789abcdef";
        assert_eq!(VikingUri::normalize_file_reference(id), id);
        assert_eq!(VikingUri::normalize(id), format!("viking://{id}"));
        let upper = "0123456789ABCDEF0123456789ABCDEF";
        assert_eq!(
            VikingUri::normalize_file_reference(upper),
            format!("viking://{upper}")
        );
    }
}
