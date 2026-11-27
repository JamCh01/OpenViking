// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Task-local `X-OpenViking-Actor-Peer` request scoping for concurrent async tasks.

use std::future::Future;

use crate::error::{OpenVikingError, Result};

tokio::task_local! {
    static ACTOR_PEER_ID: Option<String>;
}

fn normalize_actor_peer_id(actor_peer_id: Option<&str>) -> Result<Option<String>> {
    let Some(raw) = actor_peer_id else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(OpenVikingError::invalid_argument(
            "actor_peer_id must not be empty",
        ));
    }
    Ok(Some(trimmed.to_owned()))
}

/// Runs `future` with a task-local `actor_peer_id` scope applied to outgoing OpenViking requests.
///
/// Passing `Some("assistant-a")` trims surrounding whitespace and overrides the `X-OpenViking-Actor-Peer`
/// header for every request awaited inside `future`. Passing `None` clears an enclosing task-local
/// override for the duration of `future` (falling back to the client's configured `actor_peer_id`, if any).
pub async fn use_actor_peer<F>(actor_peer_id: Option<&str>, future: F) -> Result<F::Output>
where
    F: Future,
{
    let normalized = normalize_actor_peer_id(actor_peer_id)?;
    Ok(ACTOR_PEER_ID.scope(normalized, future).await)
}

/// Returns the active task-local `actor_peer_id` in the current Tokio execution context, if any.
pub fn get_actor_peer_id() -> Option<String> {
    ACTOR_PEER_ID.try_with(Clone::clone).ok().flatten()
}
