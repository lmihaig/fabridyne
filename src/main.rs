mod json_io;
mod simulator;

use json_io::{parse_instructions, save_log};
use simulator::Simulator;
use std::env;
use std::process;

fn main() {
    // Expect two commandline arguments: input file and output file.
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <input.json> <output.json>", args[0]);
        process::exit(1);
    }
    let input_path = &args[1];
    let output_path = &args[2];

    // 0. Parse JSON to get the program.
    let program = parse_instructions(input_path);
    println!("Program loaded. {} instructions.", program.len());

    let mut sim = Simulator::new(program);

    // 1. Dump the state of the reset system.
    sim.dump_state_into_log();

    // 2. Cycle-by-cycle simulation loop.
    while !sim.done() {
        sim.simulate_cycle();
        sim.dump_state_into_log();
    }

    // 3. Save the output JSON log.
    let log_as_json: Vec<serde_json::Value> = sim
        .log
        .iter()
        .map(|state| serde_json::to_value(state).unwrap())
        .collect();
    save_log(output_path, &log_as_json);
    println!("Simulation log saved to {}", output_path);
}
