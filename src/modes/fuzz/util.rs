use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore as _, SeedableRng as _};

use crate::{Config, ExitStatus, FozzyError, FozzyResult, MemoryOptions};

use super::{FuzzTarget, execute_target};

pub(crate) fn mutate_bytes(buf: &mut Vec<u8>, rng: &mut ChaCha20Rng, max_len: usize) {
    let choice = (rng.next_u64() % 4) as u8;
    match choice {
        0 => bitflip(buf.as_mut_slice(), rng),
        1 => insert_byte(buf, rng, max_len),
        2 => delete_byte(buf, rng),
        _ => overwrite_byte(buf, rng),
    }
}

pub(crate) fn minimize_input(
    config: &Config,
    target: &FuzzTarget,
    input: &[u8],
    max_len: usize,
    target_status: ExitStatus,
    scenario_memory: &MemoryOptions,
) -> FozzyResult<Vec<u8>> {
    let mut best = input.to_vec();
    let mut chunk = best.len().max(1).div_ceil(2);
    while chunk > 0 && best.len() > 1 {
        let mut improved = false;
        let mut index = 0usize;
        while index < best.len() {
            let mut trial = best.clone();
            let end = (index + chunk).min(trial.len());
            trial.drain(index..end);
            if trial.is_empty() {
                index += chunk;
                continue;
            }
            if trial.len() > max_len {
                index += chunk;
                continue;
            }
            let exec = execute_target(config, target, &trial, scenario_memory)?;
            if crate::shrink_status_matches(target_status, exec.status) {
                best = trial;
                improved = true;
                continue;
            }
            index += chunk;
        }

        if !improved {
            if chunk == 1 {
                break;
            }
            chunk = chunk.div_ceil(2);
        }
    }
    Ok(best)
}

pub(crate) fn stable_edge(label: &str) -> u64 {
    let hash = blake3::hash(label.as_bytes());
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&hash.as_bytes()[..8]);
    u64::from_le_bytes(bytes)
}

pub(crate) fn seed_from_input(input: &[u8]) -> u64 {
    let hash = blake3::hash(input);
    let mut out = [0u8; 8];
    out.copy_from_slice(&hash.as_bytes()[..8]);
    u64::from_le_bytes(out)
}

pub(crate) fn gen_seed() -> u64 {
    let mut seed = [0u8; 8];
    rand_core::OsRng.fill_bytes(&mut seed);
    u64::from_le_bytes(seed)
}

pub(crate) fn rng_from_seed(seed: u64) -> ChaCha20Rng {
    let seed_bytes = blake3::hash(&seed.to_le_bytes()).as_bytes().to_owned();
    let mut seed32 = [0u8; 32];
    seed32.copy_from_slice(&seed_bytes[..32]);
    ChaCha20Rng::from_seed(seed32)
}

pub(crate) fn hex_decode(s: &str) -> FozzyResult<Vec<u8>> {
    let s = s.trim();
    if !s.len().is_multiple_of(2) {
        return Err(FozzyError::Trace("invalid hex length".to_string()));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for index in (0..bytes.len()).step_by(2) {
        let hi = hex_val(bytes[index])?;
        let lo = hex_val(bytes[index + 1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn bitflip(buf: &mut [u8], rng: &mut ChaCha20Rng) {
    if buf.is_empty() {
        return;
    }
    let idx = (rng.next_u64() as usize) % buf.len();
    let bit = 1u8 << ((rng.next_u64() as usize) % 8);
    buf[idx] ^= bit;
}

fn insert_byte(buf: &mut Vec<u8>, rng: &mut ChaCha20Rng, max_len: usize) {
    if buf.len() >= max_len {
        return;
    }
    let idx = if buf.is_empty() {
        0
    } else {
        (rng.next_u64() as usize) % (buf.len() + 1)
    };
    let value = (rng.next_u64() & 0xFF) as u8;
    buf.insert(idx, value);
}

fn delete_byte(buf: &mut Vec<u8>, rng: &mut ChaCha20Rng) {
    if buf.is_empty() {
        return;
    }
    let idx = (rng.next_u64() as usize) % buf.len();
    buf.remove(idx);
}

fn overwrite_byte(buf: &mut Vec<u8>, rng: &mut ChaCha20Rng) {
    if buf.is_empty() {
        buf.push((rng.next_u64() & 0xFF) as u8);
        return;
    }
    let idx = (rng.next_u64() as usize) % buf.len();
    buf[idx] = (rng.next_u64() & 0xFF) as u8;
}

fn hex_val(byte: u8) -> FozzyResult<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(FozzyError::Trace("invalid hex character".to_string())),
    }
}
