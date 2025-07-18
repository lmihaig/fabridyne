use crate::json_io::serialize_decoded_pcs;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DecodedInstructionEntry {
    #[serde(rename = "PC")]
    pub pc: u64,
    #[serde(skip_serializing)]
    pub op: String,
    #[serde(skip_serializing)]
    pub is_imm: bool,
    #[serde(skip_serializing)]
    pub dest: String,
    #[serde(skip_serializing)]
    pub src1: String,
    #[serde(skip_serializing)]
    pub src2: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActiveEntry {
    #[serde(rename = "Done")]
    pub done: bool,
    #[serde(rename = "Exception")]
    pub exception: bool,
    #[serde(rename = "LogicalDestination")]
    pub logical_destination: u32,
    #[serde(rename = "OldDestination")]
    pub old_destination: u32,
    #[serde(rename = "PC")]
    pub pc: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct IntegerQueueEntry {
    #[serde(rename = "DestRegister")]
    pub dest_register: u32,
    #[serde(rename = "OpAIsReady")]
    pub op_a_is_ready: bool,
    #[serde(rename = "OpARegTag")]
    pub op_a_reg_tag: u32,
    #[serde(rename = "OpAValue")]
    pub op_a_value: u64,
    #[serde(rename = "OpBIsReady")]
    pub op_b_is_ready: bool,
    #[serde(rename = "OpBRegTag")]
    pub op_b_reg_tag: u32,
    #[serde(rename = "OpBValue")]
    pub op_b_value: u64,
    #[serde(rename = "OpCode")]
    pub op_code: String,
    #[serde(rename = "PC")]
    pub pc: u64,
}

pub struct Alu {
    pub forwarding: Option<(u32, u64, u64, bool)>,
    pipeline_stage1: Option<(u32, u64, u64, bool)>,
    instruction_in_flight: Option<IntegerQueueEntry>,
}

impl Alu {
    pub fn new() -> Self {
        Self {
            forwarding: None,
            pipeline_stage1: None,
            instruction_in_flight: None,
        }
    }
    pub fn is_free(&self) -> bool {
        self.instruction_in_flight.is_none()
    }
    pub fn push_instr(&mut self, instr: IntegerQueueEntry) {
        self.instruction_in_flight = Some(instr);
    }
    pub fn execute(&mut self) {
        self.forwarding = self.pipeline_stage1.take();
        if let Some(instr) = self.instruction_in_flight.take() {
            let (r, pc, a, b, op) = (
                instr.dest_register,
                instr.pc,
                instr.op_a_value,
                instr.op_b_value,
                instr.op_code.as_str(),
            );
            let (mut ans, mut exception) = (0, false);
            match op {
                "add" | "addi" => ans = a.wrapping_add(b),
                "sub" => ans = a.wrapping_sub(b),
                "mulu" => ans = a.wrapping_mul(b),
                "divu" => {
                    if b == 0 {
                        exception = true;
                    } else {
                        ans = a / b;
                    }
                }
                "remu" => {
                    if b == 0 {
                        exception = true;
                    } else {
                        ans = a % b;
                    }
                }
                _ => panic!("Undefined op: {}", op),
            }
            self.pipeline_stage1 = Some((r, ans, pc, exception));
        }
    }
    fn reset(&mut self) {
        *self = Self::new();
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SimulatorState {
    #[serde(rename = "PC")]
    pub pc: u64,
    #[serde(rename = "PhysicalRegisterFile")]
    pub physical_register_file: Vec<u64>,
    #[serde(rename = "DecodedPCs", serialize_with = "serialize_decoded_pcs")]
    pub decoded_pcs: Vec<DecodedInstructionEntry>,
    #[serde(rename = "ExceptionPC")]
    pub exception_pc: u64,
    #[serde(rename = "Exception")]
    pub exception: bool,
    #[serde(rename = "RegisterMapTable")]
    pub register_map_table: Vec<u32>,
    #[serde(rename = "FreeList")]
    pub free_list: VecDeque<u32>,
    #[serde(rename = "BusyBitTable")]
    pub busy_bit_table: Vec<bool>,
    #[serde(rename = "ActiveList")]
    pub active_list: VecDeque<ActiveEntry>,
    #[serde(rename = "IntegerQueue")]
    pub integer_queue: Vec<IntegerQueueEntry>,
    #[serde(skip_serializing)]
    pub backpressure: bool,
}

impl Default for SimulatorState {
    fn default() -> Self {
        Self {
            pc: 0,
            physical_register_file: vec![0; 64],
            decoded_pcs: Vec::new(),
            exception_pc: 0,
            exception: false,
            register_map_table: (0..32).collect(),
            free_list: (32..64).collect::<VecDeque<u32>>(),
            busy_bit_table: vec![false; 64],
            active_list: VecDeque::new(),
            integer_queue: Vec::new(),
            backpressure: false,
        }
    }
}

pub struct Simulator {
    pub program: Vec<String>,
    pub state: SimulatorState,
    pub log: Vec<SimulatorState>,
    pub alus: Vec<Alu>,
}

impl Simulator {
    pub fn new(program: Vec<String>) -> Simulator {
        Self {
            program,
            state: SimulatorState::default(),
            log: Vec::new(),
            alus: vec![Alu::new(), Alu::new(), Alu::new(), Alu::new()],
        }
    }
    pub fn dump_state_into_log(&mut self) {
        self.log.push(self.state.clone());
    }

    pub fn done(&self) -> bool {
        let pipeline_empty = self.state.active_list.is_empty()
            && self.state.integer_queue.is_empty()
            && self.state.decoded_pcs.is_empty();

        if !pipeline_empty {
            return false;
        }

        if self.state.pc == 0x10000 {
            // Exception scenario: terminate only after the cooldown cycle.
            return !self.state.exception;
        } else {
            // Normal scenario: terminate if PC is past the end of the program.
            return self.state.pc as usize >= self.program.len();
        }
    }

    pub fn simulate_cycle(&mut self) {
        let pipeline_stalled = self.commit();

        if !pipeline_stalled {
            self.execute();
            self.issue();
            self.rename_and_dispatch();
            self.fetch_and_decode();
        }
    }

    pub fn fetch_and_decode(&mut self) {
        if self.state.backpressure || self.state.exception {
            return;
        }
        for _ in 0..4 {
            if self.state.pc as usize >= self.program.len() {
                break;
            }
            let pc = self.state.pc;
            let instr_line = self.program[pc as usize].clone();
            let parts: Vec<&str> = instr_line.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }
            let raw_op = parts[0];
            self.state.decoded_pcs.push(DecodedInstructionEntry {
                pc,
                op: raw_op.trim_end_matches('i').to_string(),
                is_imm: raw_op.ends_with('i'),
                dest: parts[1].trim_end_matches(',').to_string(),
                src1: parts[2].trim_end_matches(',').to_string(),
                src2: parts[3].to_string(),
            });
            self.state.pc += 1;
        }
    }

    pub fn rename_and_dispatch(&mut self) {
        let num_instr = self.state.decoded_pcs.len();
        self.state.backpressure = self.state.integer_queue.len() + num_instr > 32
            || self.state.active_list.len() + num_instr > 32
            || self.state.free_list.len() < num_instr;
        if self.state.backpressure || num_instr == 0 {
            return;
        }
        for instr in std::mem::take(&mut self.state.decoded_pcs) {
            let (op_a_is_ready, op_a_reg_tag, op_a_value) =
                self.get_operand_state(&instr.src1, false);
            let (op_b_is_ready, op_b_reg_tag, op_b_value) =
                self.get_operand_state(&instr.src2, instr.is_imm);
            let arch_dest: u32 = instr.dest[1..].parse().unwrap();
            let old_phys_dest = self.state.register_map_table[arch_dest as usize];
            let new_phys_dest = self.state.free_list.pop_front().unwrap();
            self.state.register_map_table[arch_dest as usize] = new_phys_dest;
            self.state.busy_bit_table[new_phys_dest as usize] = true;
            self.state.active_list.push_back(ActiveEntry {
                done: false,
                exception: false,
                logical_destination: arch_dest,
                old_destination: old_phys_dest,
                pc: instr.pc,
            });
            self.state.integer_queue.push(IntegerQueueEntry {
                dest_register: new_phys_dest,
                op_a_is_ready,
                op_a_reg_tag,
                op_a_value,
                op_b_is_ready,
                op_b_reg_tag,
                op_b_value,
                op_code: instr.op,
                pc: instr.pc,
            });
        }
    }

    fn get_operand_state(&self, src: &str, is_imm: bool) -> (bool, u32, u64) {
        if is_imm {
            return (true, 0, src.parse().unwrap());
        }
        let arch_reg: usize = src[1..].parse().unwrap();
        let phys_reg = self.state.register_map_table[arch_reg];
        if self.state.busy_bit_table[phys_reg as usize] {
            (false, phys_reg, 0)
        } else {
            (
                true,
                0,
                self.state.physical_register_file[phys_reg as usize],
            )
        }
    }

    pub fn issue(&mut self) {
        let mut ready_instr: Vec<_> = self
            .state
            .integer_queue
            .iter()
            .filter(|i| i.op_a_is_ready && i.op_b_is_ready)
            .cloned()
            .collect();
        ready_instr.sort_by_key(|k| k.pc);
        let mut issued = HashSet::new();
        for instr in ready_instr {
            if let Some(alu) = self.alus.iter_mut().find(|a| a.is_free()) {
                alu.push_instr(instr.clone());
                issued.insert(instr);
            }
        }
        self.state.integer_queue.retain(|i| !issued.contains(i));
    }

    pub fn execute(&mut self) {
        for alu in self.alus.iter_mut() {
            alu.execute();
        }
        for alu in &self.alus {
            if let Some((reg, val, pc, exception)) = alu.forwarding {
                if let Some(entry) = self.state.active_list.iter_mut().find(|e| e.pc == pc) {
                    entry.done = true;
                    entry.exception = exception;
                }
                if !exception {
                    self.state.physical_register_file[reg as usize] = val;
                    self.state.busy_bit_table[reg as usize] = false;
                    for entry in self.state.integer_queue.iter_mut() {
                        if !entry.op_a_is_ready && entry.op_a_reg_tag == reg {
                            entry.op_a_is_ready = true;
                            entry.op_a_value = val;
                            entry.op_a_reg_tag = 0;
                        }
                        if !entry.op_b_is_ready && entry.op_b_reg_tag == reg {
                            entry.op_b_is_ready = true;
                            entry.op_b_value = val;
                            entry.op_b_reg_tag = 0;
                        }
                    }
                }
            }
        }
    }

    // Returns true if the pipeline should be stalled for this cycle
    pub fn commit(&mut self) -> bool {
        if self.state.exception {
            if self.state.active_list.is_empty() {
                self.state.exception = false;
                return false;
            }

            for _ in 0..4 {
                if let Some(entry) = self.state.active_list.pop_back() {
                    let new_phys_dest =
                        self.state.register_map_table[entry.logical_destination as usize];
                    self.state.register_map_table[entry.logical_destination as usize] =
                        entry.old_destination;
                    self.state.free_list.push_back(new_phys_dest);
                    self.state.busy_bit_table[new_phys_dest as usize] = false;
                } else {
                    break;
                }
            }
            return true;
        }

        // Normal commit.
        for _ in 0..4 {
            if let Some(entry) = self.state.active_list.front() {
                if !entry.done {
                    break;
                }

                if entry.exception {
                    self.state.exception_pc = entry.pc;
                    self.state.pc = 0x10000;
                    self.state.decoded_pcs.clear();
                    self.state.integer_queue.clear();
                    for alu in self.alus.iter_mut() {
                        alu.reset();
                    }
                    self.state.exception = true;
                    return true;
                }

                let committed_entry = self.state.active_list.pop_front().unwrap();
                self.state
                    .free_list
                    .push_back(committed_entry.old_destination);
            } else {
                break;
            }
        }
        false
    }
}
