use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore as _, SeedableRng as _};

use crate::{Config, ExitStatus, HeapBudgetPolicy, ProfileCaptureLevel};

pub(super) fn heap_budget_policy(config: &Config) -> HeapBudgetPolicy {
    HeapBudgetPolicy {
        alloc_bytes_budget: config.profile_heap_alloc_budget,
        in_use_bytes_budget: config.profile_heap_in_use_budget,
    }
}

pub(super) fn should_emit_heavy_artifacts(status: ExitStatus, explicit_request: bool) -> bool {
    explicit_request
        || status != ExitStatus::Pass
        || std::env::var("FOZZY_ARTIFACTS_FULL")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

pub(super) fn should_emit_full_profile(profile_capture: ProfileCaptureLevel) -> bool {
    matches!(profile_capture, ProfileCaptureLevel::Full)
}

pub(super) fn bump(next_id: &mut u64) -> u64 {
    let id = *next_id;
    *next_id = next_id.saturating_add(1);
    id
}

pub(super) fn ordered_pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a <= b { (a, b) } else { (b, a) }
}

pub(super) fn gen_seed() -> u64 {
    let mut seed = [0u8; 8];
    rand_core::OsRng.fill_bytes(&mut seed);
    u64::from_le_bytes(seed)
}

pub(super) fn rng_from_seed(seed: u64) -> ChaCha20Rng {
    let seed_bytes = blake3::hash(&seed.to_le_bytes()).as_bytes().to_owned();
    let mut seed32 = [0u8; 32];
    seed32.copy_from_slice(&seed_bytes[..32]);
    ChaCha20Rng::from_seed(seed32)
}

pub(super) fn stable_edge(label: &str) -> u64 {
    let h = blake3::hash(label.as_bytes());
    let mut b = [0u8; 8];
    b.copy_from_slice(&h.as_bytes()[..8]);
    u64::from_le_bytes(b)
}
