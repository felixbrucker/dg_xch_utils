extern crate core;

use crate::verifier::validate_proof;
use dg_xch_core::blockchain::proof_of_space::{
    calculate_pos_challenge, passes_plot_filter, ProofOfSpace,
};
use dg_xch_core::blockchain::sized_bytes::Bytes32;
use dg_xch_core::consensus::constants::ConsensusConstants;
use log::warn;

pub mod chacha8;
pub mod constants;
pub mod encoding;
pub mod entry_sizes;
pub mod f_calc;
pub mod finite_state_entropy;
pub mod plots;
pub mod utils;
pub mod verifier;

fn _version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
fn _pkg_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

pub fn version() -> String {
    format!("{}: {}", _pkg_name(), _version())
}

#[test]
fn test_version() {
    println!("{}", version());
}

pub fn verify_and_get_quality_string(
    pos: &ProofOfSpace,
    constants: &ConsensusConstants,
    original_challenge_hash: &Bytes32,
    signage_point: &Bytes32,
) -> Option<Bytes32> {
    if pos.pool_public_key.is_none() && pos.pool_contract_puzzle_hash.is_none() {
        warn!("Failed to Verify ProofOfSpace: null value for pool_public_key and pool_contract_puzzle_hash");
        return None;
    }
    if pos.pool_public_key.is_some() && pos.pool_contract_puzzle_hash.is_some() {
        warn!("Failed to Verify ProofOfSpace: Non Null value for both for pool_public_key and pool_contract_puzzle_hash");
        return None;
    }
    if pos.size < constants.min_plot_size {
        warn!("Failed to Verify ProofOfSpace: Plot failed MIN_PLOT_SIZE");
        return None;
    }
    if pos.size > constants.max_plot_size {
        warn!("Failed to Verify ProofOfSpace: Plot failed MAX_PLOT_SIZE");
        return None;
    }
    if let Some(plot_id) = pos.get_plot_id() {
        if pos.challenge
            != calculate_pos_challenge(&plot_id, original_challenge_hash, signage_point)
        {
            warn!("Failed to Verify ProofOfSpace: New challenge is not challenge");
            return None;
        }
        if !passes_plot_filter(constants, &plot_id, original_challenge_hash, signage_point) {
            warn!("Failed to Verify ProofOfSpace: Plot Failed to Pass Filter");
            return None;
        }
        match validate_proof(
            plot_id.to_sized_bytes(),
            pos.size,
            pos.proof.as_ref(),
            pos.challenge.as_ref(),
        ) {
            Ok(q) => Some(q),
            Err(e) => {
                warn!("Failed to Validate Proof: {:?}", e);
                None
            }
        }
    } else {
        None
    }
}
