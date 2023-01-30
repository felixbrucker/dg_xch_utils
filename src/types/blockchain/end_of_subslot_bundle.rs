use crate::types::blockchain::challenge_chain_subslot::ChallengeChainSubSlot;
use crate::types::blockchain::infused_challenge_chain_subslot::InfusedChallengeChainSubSlot;
use crate::types::blockchain::reward_chain_subslot::RewardChainSubSlot;
use crate::types::blockchain::subslot_proofs::SubSlotProofs;

pub struct EndOfSubSlotBundle {
    pub challenge_chain: ChallengeChainSubSlot,
    pub infused_challenge_chain: Option<InfusedChallengeChainSubSlot>,
    pub reward_chain: RewardChainSubSlot,
    pub proofs: SubSlotProofs,
}
