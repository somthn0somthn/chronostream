use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct StreamConfig {
    pub sender: Pubkey,
    pub receiver: Pubkey,
    pub flow_rate: i64,
    pub static_balance: u64,
    pub start_time: i64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum StreamInstruction {
    Initialize {
        flow_rate: i64,
        initial_balance: u64,
    },
    Terminate,
}

impl StreamConfig {
    pub fn initialize(
        sender: Pubkey,
        receiver: Pubkey,
        flow_rate: i64,
        initial_balance: u64,
        start_time: i64,
    ) -> Self {
        StreamConfig {
            sender,
            receiver,
            flow_rate,
            static_balance: initial_balance,
            start_time,
        }
    }
}