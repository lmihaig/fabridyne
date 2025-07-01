use std::fs;
use std::process;

/// Parses the input JSON file and returns a vector of instruction strings.
/// Expects the JSON file to contain an array of instructions.
pub fn parse_instructions(input_path: &str) -> Vec<String> {
    let input_data = fs::read_to_string(input_path).unwrap_or_else(|err| {
        eprintln!("Failed to read input file: {}", err);
        process::exit(1);
    });

    let instructions: serde_json::Value = serde_json::from_str(&input_data).unwrap_or_else(|err| {
        eprintln!("Failed to parse JSON input: {}", err);
        process::exit(1);
    });

    if let Some(array) = instructions.as_array() {
        array
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_string())
            .collect()
    } else {
        eprintln!("Input JSON is not an array.");
        vec![]
    }
}

/// Saves the simulation log (a vector of JSON states) to the specified output file.
pub fn save_log(output_path: &str, log: &Vec<serde_json::Value>) {
    let output = serde_json::to_string_pretty(&log).unwrap_or_else(|err| {
        eprintln!("Failed to serialize simulation log: {}", err);
        process::exit(1);
    });
    fs::write(output_path, output).unwrap_or_else(|err| {
        eprintln!("Failed to write output file: {}", err);
        process::exit(1);
    });
}

use serde::Serialize;
use serde::ser::Serializer;

use crate::simulator::DecodedInstructionEntry;

pub fn serialize_decoded_pcs<S>(
    decoded: &Vec<DecodedInstructionEntry>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // Map each DecodedInstructionEntry to its pc field.
    let pcs: Vec<u64> = decoded.iter().map(|d| d.pc).collect();
    pcs.serialize(serializer)
}
