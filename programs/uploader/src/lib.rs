#![feature(array_methods)]

pub mod instruction;

mod processor;
pub use processor::process_instruction;

mod state;
