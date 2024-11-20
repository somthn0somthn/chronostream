use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::state::{StreamConfig, StreamInstruction};
use borsh::{BorshDeserialize, BorshSerialize};

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = StreamInstruction::try_from_slice(instruction_data)?;

    match instruction {
        StreamInstruction::Initialize {
            flow_rate,
            initial_balance,
        } => process_initialize(program_id, accounts, flow_rate, initial_balance),
        StreamInstruction::Terminate => process_terminate(program_id, accounts),
    }
}

fn process_initialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    flow_rate: i64,
    initial_balance: u64,
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();

    let stream_account = next_account_info(accounts_iter)?;
    let sender = next_account_info(accounts_iter)?;
    let receiver = next_account_info(accounts_iter)?;

    // Validate account ownership
    if stream_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Validate signer
    if !sender.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Get current timestamp for stream start
    let start_time = Clock::get()?.unix_timestamp;

    // Create and initialize the stream
    let stream = StreamConfig::initialize(
        *sender.key,
        *receiver.key,
        flow_rate,
        initial_balance,
        start_time,
    );

    // Serialize and store the stream data
    stream.serialize(&mut &mut stream_account.data.borrow_mut()[..])?;

    msg!(
        "Stream initialized: flow_rate={}, initial_balance={}",
        flow_rate,
        initial_balance
    );
    Ok(())
}

fn process_terminate(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();

    let stream_account = next_account_info(accounts_iter)?;
    let sender = next_account_info(accounts_iter)?;
    let receiver = next_account_info(accounts_iter)?;

    // Validate account ownership
    if stream_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Deserialize the stream data
    let mut stream = StreamConfig::try_from_slice(&stream_account.data.borrow())?;

    // Verify either sender or receiver signed
    if !((sender.is_signer && stream.sender == *sender.key)
        || (receiver.is_signer && stream.receiver == *receiver.key))
    {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Calculate streamed amount
    let current_time = Clock::get()?.unix_timestamp;
    let time_elapsed = current_time - stream.start_time;
    let amount_streamed = (time_elapsed * stream.flow_rate) as u64;

    // Update balance
    if amount_streamed > stream.static_balance {
        stream.static_balance = 0;
    } else {
        stream.static_balance -= amount_streamed;
    }

    // Save updated stream data
    stream.serialize(&mut &mut stream_account.data.borrow_mut()[..])?;

    msg!(
        "Stream terminated by {}: remaining_balance={}",
        if sender.is_signer {
            "sender"
        } else {
            "receiver"
        },
        stream.static_balance
    );
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use solana_program::{clock::Clock, clock::Epoch};
    use std::mem;

    pub struct Test;

    impl Test {
        pub fn get_clock() -> Clock {
            Clock {
                slot: 1,
                epoch_start_timestamp: 1,
                epoch: 1,
                leader_schedule_epoch: 1,
                unix_timestamp: 1000,
            }
        }

        pub fn time_warp(seconds_to_advance: i64) -> Clock {
            let base_timestamp = 1000;
            Clock {
                slot: 1000,
                epoch_start_timestamp: 1,
                epoch: 1,
                leader_schedule_epoch: 1,
                unix_timestamp: base_timestamp + seconds_to_advance,
            }
        }

        pub const ONE_HOUR: i64 = 3600;
        pub const ONE_DAY: i64 = 86400;
        pub const ONE_WEEK: i64 = 86400 * 7;
        pub const ONE_MONTH: i64 = 86400 * 30;
    }

    #[test]
    fn test_initialization() {
        let program_id = Pubkey::default();
        let sender_key = Pubkey::default();
        let receiver_key = Pubkey::new_unique();

        // Create the stream account
        let mut stream_lamports = 0;
        let mut stream_data = vec![0; mem::size_of::<StreamConfig>()];
        let owner = program_id;
        let binding = Pubkey::new_unique();
        let stream_account = AccountInfo::new(
            &binding,
            false,
            true,
            &mut stream_lamports,
            &mut stream_data,
            &owner,
            false,
            Epoch::default(),
        );

        let mut sender_lamports = 0;
        let mut sender_data = vec![];
        let sender_account = AccountInfo::new(
            &sender_key,
            true,
            false,
            &mut sender_lamports,
            &mut sender_data,
            &owner,
            false,
            Epoch::default(),
        );

        let mut receiver_lamports = 0;
        let mut receiver_data = vec![];
        let receiver_account = AccountInfo::new(
            &receiver_key,
            false,
            false,
            &mut receiver_lamports,
            &mut receiver_data,
            &owner,
            false,
            Epoch::default(),
        );

        let accounts = vec![stream_account, sender_account, receiver_account];

        let init_instr = StreamInstruction::Initialize {
            flow_rate: 100,
            initial_balance: 1000,
        };

        let mut instr_data = vec![];
        init_instr.serialize(&mut instr_data).unwrap();

        // Mock the Clock for our test
        solana_program::program_stubs::set_syscall_stubs(Box::new(TestSyscallStubs {
            clock: Test::get_clock(),
        }));

        assert_eq!(
            process_instruction(&program_id, &accounts, &instr_data),
            Ok(())
        );

        let stream = StreamConfig::try_from_slice(&accounts[0].data.borrow()).unwrap();
        println!("flow rate {}", &stream.flow_rate);
        assert_eq!(stream.flow_rate, 100);
        println!("static balance {}", &stream.static_balance);
        assert_eq!(stream.static_balance, 1000);
        println!("sender {}", &stream.sender);
        assert_eq!(stream.sender, sender_key);
        println!("receiver {}", &stream.receiver);
        assert_eq!(stream.receiver, receiver_key);
        println!("start time {}", &stream.start_time);
        assert_eq!(stream.start_time, 1000); // Should match our mocked timestamp
    }

    #[test]
    fn test_termination() {
        let program_id = Pubkey::default();
        let sender_key = Pubkey::default();
        let receiver_key = Pubkey::new_unique();

        let mut stream_lamports = 0;
        let mut stream_data = vec![0; mem::size_of::<StreamConfig>()];
        let owner = program_id;
        let binding = Pubkey::new_unique();
        let stream_account = AccountInfo::new(
            &binding,
            false,
            true,
            &mut stream_lamports,
            &mut stream_data,
            &owner,
            false,
            Epoch::default(),
        );

        let mut sender_lamports = 0;
        let mut sender_data = vec![];
        let mut sender_account = AccountInfo::new(
            &sender_key,
            true,
            false,
            &mut sender_lamports,
            &mut sender_data,
            &owner,
            false,
            Epoch::default(),
        );

        let mut receiver_lamports = 0;
        let mut receiver_data = vec![];
        let mut receiver_account = AccountInfo::new(
            &receiver_key,
            false,
            false,
            &mut receiver_lamports,
            &mut receiver_data,
            &owner,
            false,
            Epoch::default(),
        );

        // Initialize the stream
        {
            let init_accounts = vec![
                stream_account.clone(),
                sender_account.clone(),
                receiver_account.clone(),
            ];

            let init_instr = StreamInstruction::Initialize {
                flow_rate: 100,
                initial_balance: 1000,
            };

            solana_program::program_stubs::set_syscall_stubs(Box::new(TestSyscallStubs {
                clock: Test::get_clock(),
            }));

            let mut init_data = vec![];
            init_instr.serialize(&mut init_data).unwrap();

            assert_eq!(
                process_instruction(&program_id, &init_accounts, &init_data),
                Ok(())
            );
        }

        solana_program::program_stubs::set_syscall_stubs(Box::new(TestSyscallStubs {
            clock: Test::time_warp(Test::ONE_DAY),
        }));

        sender_account.is_signer = false;
        receiver_account.is_signer = true;

        // Terminate the stream
        {
            let term_accounts = vec![
                stream_account.clone(),
                sender_account.clone(),
                receiver_account.clone(),
            ];

            let term_instr = StreamInstruction::Terminate;
            let mut term_data = vec![];
            term_instr.serialize(&mut term_data).unwrap();

            assert_eq!(
                process_instruction(&program_id, &term_accounts, &term_data),
                Ok(())
            );
        }

        let stream = StreamConfig::try_from_slice(&stream_account.data.borrow()).unwrap();
        println!("flow rate term {}", &stream.flow_rate);
        assert_eq!(stream.flow_rate, 100);
        println!("static balance term {}", &stream.static_balance);
        assert_eq!(stream.static_balance, 0);
        println!("sender term {}", &stream.sender);
        assert_eq!(stream.sender, sender_key);
        println!("receiver term {}", &stream.receiver);
        assert_eq!(stream.receiver, receiver_key);
        println!("start term time {}", &stream.start_time);
        assert_eq!(stream.start_time, 1000);
    }
}

#[cfg(test)]
pub struct TestSyscallStubs {
    clock: Clock,
}

#[cfg(test)]
impl solana_program::program_stubs::SyscallStubs for TestSyscallStubs {
    fn sol_get_clock_sysvar(&self, var_addr: *mut u8) -> u64 {
        unsafe {
            *(var_addr as *mut Clock) = self.clock.clone();
        }
        0
    }
}

//TODO :: Add frontend end testing suite
