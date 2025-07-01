use crate::json_io::serialize_decoded_pcs;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DecodedInstructionEntry {
    #[serde(rename = "PC")]
    pub pc: u64,
    #[serde(skip_serializing)]
    pub op: String,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
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
    pub instruction_queue: VecDeque<Option<IntegerQueueEntry>>,
    pipeline_stage1: Option<(u32, u64, u64, bool)>,
}

impl Alu {
    pub fn new() -> Self {
        let instruction_queue = VecDeque::new();

        Self {
            forwarding: None,
            instruction_queue,
            pipeline_stage1: None,
        }
    }

    /// Pops the oldest instruction from the ALU queue (from the front).
    pub fn pop_instr(&mut self) -> Option<IntegerQueueEntry> {
        self.instruction_queue.pop_front().flatten()
    }
    /// Pushes a new instruction onto the ALU queue (to the back).
    pub fn push_instr(&mut self, instr: Option<IntegerQueueEntry>) {
        if let Some(entry) = instr {
            // Wrap the entry in Some and push it.
            self.instruction_queue.push_back(Some(entry));
        }
    }

    pub fn execute(&mut self) {
        self.forwarding = self.pipeline_stage1.take();

        if let Some(instr) = self.pop_instr() {
            let r = instr.dest_register;
            let a = instr.op_a_value;
            let b = instr.op_b_value;
            let op = instr.op_code.as_str();
            let pc = instr.pc;
            let mut ans: u64 = 0;
            let mut exception = false;

            match op {
                "add" | "addi" => {
                    let a_signed = a as i64;
                    let b_signed = b as i64;
                    let result = a_signed.wrapping_add(b_signed);
                    ans = result as u64;
                }
                "sub" => {
                    let a_signed = a as i64;
                    let b_signed = b as i64;
                    let result = a_signed.wrapping_sub(b_signed);
                    ans = result as u64;
                }
                "mulu" => {
                    ans = a.wrapping_mul(b);
                }
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
                _ => {
                    panic!("Undefined operation: {}", op);
                }
            }
            self.pipeline_stage1 = Some((r, ans, pc, exception));
        } else {
            self.pipeline_stage1 = None
        }
    }

    fn reset(&mut self) {
        self.forwarding = None;
        self.instruction_queue.clear();
        self.pipeline_stage1 = None;
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SimulatorState {
    #[serde(rename = "PC")]
    pub pc: u64,
    #[serde(rename = "PhysicalRegisterFile")]
    pub physical_register_file: Vec<u64>, // 64 registers.
    #[serde(rename = "DecodedPCs", serialize_with = "serialize_decoded_pcs")]
    pub decoded_pcs: Vec<DecodedInstructionEntry>,
    #[serde(rename = "ExceptionPC")]
    pub exception_pc: u64,
    #[serde(rename = "Exception")]
    pub exception: bool,
    #[serde(rename = "RegisterMapTable")]
    pub register_map_table: Vec<u32>, // 32 registers.
    #[serde(rename = "FreeList")]
    pub free_list: VecDeque<u32>,
    #[serde(rename = "BusyBitTable")]
    pub busy_bit_table: Vec<bool>, // 64 booleans.
    #[serde(rename = "ActiveList")]
    pub active_list: VecDeque<ActiveEntry>,
    #[serde(rename = "IntegerQueue")]
    pub integer_queue: Vec<IntegerQueueEntry>,
    #[serde(skip_serializing)]
    pub backpressure: bool,
}

impl Default for SimulatorState {
    fn default() -> Self {
        SimulatorState {
            pc: 0,
            physical_register_file: vec![0; 64],
            decoded_pcs: Vec::new(),
            exception_pc: 0,
            exception: false,
            register_map_table: (0..32).collect(),
            free_list: (32..64).collect::<Vec<u32>>().into_iter().collect(),
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
    /// Initializes a new Simulator with the given program
    pub fn new(program: Vec<String>) -> Simulator {
        let alus = vec![Alu::new(), Alu::new(), Alu::new(), Alu::new()];
        Simulator {
            program,
            state: SimulatorState::default(),
            log: Vec::new(),
            alus,
        }
    }

    /// Dumps the current state into the simulation log
    pub fn dump_state_into_log(&mut self) {
        self.log.push(self.state.clone());
        // println!("State dumped. PC: {}", self.state.pc);
    }

    // Returns true if simulation has finished
    pub fn done(&self) -> bool {
        let pc_finished =
            (self.state.pc as usize == self.program.len()) || self.state.pc == 0x10000;
        // println!("pc_finished: {}", pc_finished);
        //
        let active_list_empty = self.state.active_list.is_empty();
        // println!("self.state.active_list.is_empty(): {}", active_list_empty);
        //
        // println!("self.state.exception: {}", self.state.exception);

        let decoded_pcs_empty = self.state.decoded_pcs.is_empty();
        // println!("self.state.decoded_pcs.is_empty(): {}", decoded_pcs_empty);

        // println!("Active list: {:?}", self.state.active_list);
        pc_finished && active_list_empty && !self.state.exception && decoded_pcs_empty
    }

    /// Simulate one cycle by executing the stages in reverse order to avoid having to copy the entire state
    pub fn simulate_cycle(&mut self) {
        // Execute the ALU stage first, as it’s modeled as asynchronous-read
        // println!("Starting ALU execution stage");
        self.execute();
        // Rest of the stages are sync
        // println!("Starting commit stage");
        self.commit();
        // println!("Starting issue stage");
        self.issue();
        // println!("Starting rename and dispatch stage");
        self.rename_and_dispatch();
        // println!("Starting fetch and decode stage");
        self.fetch_and_decode();
    }

    /// Fetch and decode stage
    pub fn fetch_and_decode(&mut self) {
        // stop if commit stage enabled exception mode, set pc exception addr and flush
        if self.state.exception {
            self.state.pc = 0x10000;
            self.state.decoded_pcs.clear();
            return;
        }

        // stop if there is backpressure or PC is in exception
        if self.state.backpressure || self.state.pc == 0x10000 {
            return;
        }

        // fetch 4 instr
        for _ in 0..4 {
            let pc = self.state.pc;
            // no instructions
            if self.state.pc as usize == self.program.len() {
                break;
            }
            self.state.pc += 1;
            let instr_line = self.program[pc as usize].clone();
            let parts: Vec<&str> = instr_line.split_whitespace().collect();
            if parts.len() < 4 {
                eprintln!("BRUH RISC IS FIXED LENGTH: {} ", instr_line);
                continue;
            }

            let op = parts[0].trim_end_matches("i").to_string();
            let dest = parts[1].trim_end_matches(",").to_string();
            let src1 = parts[2].trim_end_matches(",").to_string();
            let src2 = parts[3].to_string();

            let decoded = DecodedInstructionEntry {
                pc,
                op,
                dest,
                src1,
                src2,
            };

            self.state.decoded_pcs.push(decoded)
        }
    }

    /// Rename and dispatch stage
    pub fn rename_and_dispatch(&mut self) {
        if self.state.exception {
            return;
        }

        for alu in self.alus.iter_mut() {
            if let Some((reg, val, pc, exception)) = alu.forwarding {
                if exception {
                    for entry in self.state.active_list.iter_mut() {
                        if entry.pc == pc {
                            entry.exception = true;
                            entry.done = true;
                        }
                    }
                    continue;
                }
                self.state.physical_register_file[reg as usize] = val;
                self.state.busy_bit_table[reg as usize] = false;
            }
        }

        let num_instr = self.state.decoded_pcs.len();
        if (self.state.integer_queue.len() > 32 - num_instr)
            || (self.state.active_list.len() > 32 - num_instr)
            || (self.state.free_list.len() < num_instr)
        {
            self.state.backpressure = true;
            return;
        }

        self.state.backpressure = false;

        let instructions = std::mem::take(&mut self.state.decoded_pcs);
        // (renamed src1, renamed src2, new destination, old destination, opcode, PC, original arch reg)
        let renamed_instructions: Vec<_> = instructions
            .into_iter()
            .map(|instr| {
                let (src1, src2, old_dest, new_dest) = self.rename_registers(&instr);
                let arch_reg: u32 = instr.dest[1..]
                    .parse()
                    .expect("Invalid destination register format");
                (src1, src2, new_dest, old_dest, instr.op, instr.pc, arch_reg)
            })
            .collect();

        // For each renamed instruction, determine the ready state and dispatch it.
        for (src1, src2, new_dest, old_dest, opcode, pc, arch) in renamed_instructions {
            let (ready_a, tag_a, val_a) = if !src1.starts_with('p') {
                // Immediate value: assume the string parses directly to an integer.
                (
                    true,
                    0,
                    src1.parse::<u64>().expect("Invalid immediate value"),
                )
            } else {
                // Extract the physical register number.
                let reg: usize = src1[1..].parse().expect("Invalid physical register");
                if self.state.busy_bit_table[reg] {
                    (false, reg as u32, 0)
                } else {
                    (true, 0, self.state.physical_register_file[reg])
                }
            };

            let (ready_b, tag_b, val_b) = if !src2.starts_with('p') {
                (
                    true,
                    0,
                    src2.parse::<u64>().expect("Invalid immediate value"),
                )
            } else {
                let reg: usize = src2[1..].parse().expect("Invalid physical register");
                if self.state.busy_bit_table[reg] {
                    (false, reg as u32, 0)
                } else {
                    (true, 0, self.state.physical_register_file[reg])
                }
            };

            self.state.integer_queue.push(IntegerQueueEntry {
                dest_register: new_dest,
                op_a_is_ready: ready_a,
                op_a_reg_tag: tag_a,
                op_a_value: val_a,
                op_b_is_ready: ready_b,
                op_b_reg_tag: tag_b,
                op_b_value: val_b,
                op_code: opcode.clone(),
                pc,
            });

            self.state.active_list.push_back(ActiveEntry {
                done: false,
                exception: false,
                logical_destination: arch,
                old_destination: old_dest,
                pc,
            });
        }
    }

    pub fn rename_registers(
        &mut self,
        instr: &DecodedInstructionEntry,
    ) -> (String, String, u32, u32) {
        let src1 = if instr.src1.starts_with('x') {
            let arch_reg: usize = instr.src1[1..].parse().expect("Invalid source register");
            let phys_reg = self.state.register_map_table[arch_reg];
            format!("p{}", phys_reg)
        } else {
            instr.src1.clone()
        };

        let src2 = if instr.src2.starts_with('x') {
            let arch_reg: usize = instr.src2[1..].parse().expect("Invalid source register");
            let phys_reg = self.state.register_map_table[arch_reg];
            format!("p{}", phys_reg)
        } else {
            instr.src2.clone()
        };

        let arch_reg: usize = instr.dest[1..]
            .parse()
            .expect("Invalid destination register");

        let old_dest = self.state.register_map_table[arch_reg];

        let new_dest = self
            .state
            .free_list
            .pop_front()
            .expect("No free registers available for destination renaming");
        self.state.register_map_table[arch_reg] = new_dest;
        self.state.busy_bit_table[new_dest as usize] = true;

        (src1, src2, old_dest, new_dest)
    }

    /// Issue stage
    pub fn issue(&mut self) {
        if self.state.exception {
            return;
        }
        for alu in self.alus.iter_mut() {
            if let Some((reg, val, fp_pc, exception)) = alu.forwarding {
                if exception {
                    for entry in self.state.active_list.iter_mut() {
                        if entry.pc == fp_pc {
                            entry.done = true;
                            entry.exception = true;
                        }
                    }
                    continue;
                }

                for entry in self.state.integer_queue.iter_mut() {
                    // println!("{} {}", reg, entry.op_b_reg_tag);
                    if entry.op_b_reg_tag == reg && !entry.op_b_is_ready {
                        // println!("BEEN HERE");
                        entry.op_b_reg_tag = 0;
                        entry.op_b_is_ready = true;
                        entry.op_b_value = val;
                    }
                    if entry.op_a_reg_tag == reg && !entry.op_a_is_ready {
                        entry.op_a_reg_tag = 0;
                        entry.op_a_is_ready = true;
                        entry.op_a_value = val;
                    }
                }
                self.state.busy_bit_table[reg as usize] = false;
            }
        }

        let mut ready = Vec::new();
        let mut i = 0;
        while i < self.state.integer_queue.len() {
            if self.state.integer_queue[i].op_a_is_ready
                && self.state.integer_queue[i].op_b_is_ready
            {
                ready.push(self.state.integer_queue.remove(i));
            } else {
                i += 1;
            }
        }

        // For each ALU (up to 4), assign a ready instruction if available.
        for i in 0..std::cmp::min(4, ready.len()) {
            if let Some(curr_instr) = ready.get(i) {
                self.alus[i].push_instr(Some(curr_instr.clone()));
            }
        }
    }

    /// ALU execution stage
    pub fn execute(&mut self) {
        if self.state.exception {
            return;
        }

        for i in 0..4 {
            self.alus[i].execute();
        }
    }
    // commit stage
    pub fn commit(&mut self) {
        // If we are in exception mode, roll back up to 4 instructions
        if self.state.exception {
            if self.state.active_list.is_empty() {
                self.state.exception = false;
            } else {
                for _ in 0..4 {
                    if let Some(entry) = self.state.active_list.pop_back() {
                        let logical_dest = entry.logical_destination as usize;
                        let old_dest = entry.old_destination;
                        let curr_dest = self.state.register_map_table[logical_dest];
                        // Free the currently allocated (wrong) physical register.
                        self.state.free_list.push_back(curr_dest);
                        self.state.busy_bit_table[curr_dest as usize] = false;
                        // Restore the old mapping.
                        self.state.register_map_table[logical_dest] = old_dest;
                    }
                }
            }
            return;
        }

        // --- Normal Commit Stage: Try to commit up to 4 instructions from the active list.
        for _ in 0..4 {
            let result = self.pop_ready_instr();
            match result {
                None => break,
                Some(result) => {
                    if result.exception {
                        // Raise exception:
                        self.state.exception = true;
                        self.state.exception_pc = result.pc;
                        // Reset all ALUs.
                        for alu in self.alus.iter_mut() {
                            alu.reset()
                        }
                        // Reset the integer queue.
                        self.state.integer_queue.clear();
                        return;
                    } else {
                        // Normal commit: free the old physical register.
                        self.state.free_list.push_back(result.old_destination);
                    }
                }
            }
        }

        for alu in self.alus.iter_mut() {
            if let Some((_reg, _val, pc, exception)) = alu.forwarding {
                for entry in self.state.active_list.iter_mut() {
                    if entry.pc == pc {
                        // this happens when it shouldn't
                        entry.done = true;
                        if exception {
                            entry.exception = true;
                        }
                    }
                }
            }
        }
        // println!("{:?}", self.state.active_list);
    }

    pub fn pop_ready_instr(&mut self) -> Option<ActiveEntry> {
        if let Some(front) = self.state.active_list.front() {
            if front.done || front.exception {
                if !front.exception {
                    self.state.active_list.pop_front()
                } else {
                    Some(front.clone())
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}
