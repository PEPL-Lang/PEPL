//! Gas-metering instrumentation.
//!
//! Injects a `gas_tick` call at:
//! - Every `for` loop iteration header
//! - Every action / function call site
//! - Every `update()` call boundary
//!
//! When the counter exceeds the limit, the runtime traps.

use wasm_encoder::{Function, Instruction};

use crate::types::{GLOBAL_GAS, GLOBAL_GAS_LIMIT, IMPORT_TRAP};

/// Emit instructions that increment the gas counter and trap if exhausted.
///
/// Equivalent pseudo-code:
/// ```text
/// gas += 1
/// if gas > gas_limit { trap("gas exhausted") }
/// ```
pub fn emit_gas_tick(f: &mut Function, trap_msg_ptr: u32, trap_msg_len: u32) {
    // gas += 1
    f.instruction(&Instruction::GlobalGet(GLOBAL_GAS));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::GlobalSet(GLOBAL_GAS));

    // if gas > gas_limit → trap
    f.instruction(&Instruction::GlobalGet(GLOBAL_GAS));
    f.instruction(&Instruction::GlobalGet(GLOBAL_GAS_LIMIT));
    f.instruction(&Instruction::I32GtU);
    f.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    f.instruction(&Instruction::I32Const(trap_msg_ptr as i32));
    f.instruction(&Instruction::I32Const(trap_msg_len as i32));
    f.instruction(&Instruction::Call(IMPORT_TRAP));
    f.instruction(&Instruction::Unreachable);
    f.instruction(&Instruction::End);
}
