use colored::Colorize;
use tracing::{debug, info, trace, warn};

use crate::cpu::{Cpu, Decoder, Executor};
use crate::error::{EmulatorError, Result};
use crate::memory::{Bus, MemoryRegion, Ram, SimpleBus};
use crate::peripherals::{Gic, Timer, Uart};

#[derive(Debug, Clone)]
pub struct SocConfig {
    pub ram_base: u64,
    pub ram_size: usize,
    pub uart_base: u64,
    pub timer_base: u64,
    pub gic_dist_base: u64,
    pub gic_cpu_base: u64,
    pub entry_point: u64,
    pub max_instructions: u64,
}

impl Default for SocConfig {
    fn default() -> Self {
        Self {
            ram_base: 0x4000_0000,
            ram_size: 64 * 1024 * 1024,
            uart_base: 0x0900_0000,
            timer_base: 0x0901_0000,
            gic_dist_base: 0x0800_0000,
            gic_cpu_base: 0x0801_0000,
            entry_point: 0x4000_0000,
            max_instructions: 0,
        }
    }
}

#[derive(Debug)]
pub enum StepResult {
    Continue,
    Halted,
    Breakpoint(u16),
    Error(EmulatorError),
}

pub struct Soc {
    pub cpu: Cpu,
    pub bus: SimpleBus,
    decoder: Decoder,
    executor: Executor,
    pub gic: Gic,
    config: SocConfig,
}

impl Soc {
    pub fn new(config: SocConfig) -> Result<Self> {
        let mut bus = SimpleBus::new();

        Self::add_ram(&mut bus, &config)?;
        Self::add_uart(&mut bus, &config)?;
        Self::add_timer(&mut bus, &config)?;
        let gic = Self::add_gic(&mut bus, &config)?;

        let mut cpu = Cpu::new();
        cpu.reset(config.entry_point);

        info!(
            entry_point = format_args!("{:#x}", config.entry_point),
            ram_size = config.ram_size,
            "soc initialized"
        );

        Ok(Self { cpu, bus, decoder: Decoder::new(), executor: Executor::new(), gic, config })
    }

    fn add_ram(bus: &mut SimpleBus, config: &SocConfig) -> Result<()> {
        let ram = Ram::new("ram", config.ram_size);
        bus.add_region(MemoryRegion::new(config.ram_base, config.ram_size as u64, Box::new(ram)))
            .map_err(|e| EmulatorError::Config { message: format!("failed to map ram: {e}") })
    }

    fn add_uart(bus: &mut SimpleBus, config: &SocConfig) -> Result<()> {
        let uart = Uart::new("uart0");
        bus.add_region(MemoryRegion::new(config.uart_base, 0x1000, Box::new(uart)))
            .map_err(|e| EmulatorError::Config { message: format!("failed to map uart: {e}") })
    }

    fn add_timer(bus: &mut SimpleBus, config: &SocConfig) -> Result<()> {
        let timer = Timer::new("timer0");
        bus.add_region(MemoryRegion::new(config.timer_base, 0x1000, Box::new(timer)))
            .map_err(|e| EmulatorError::Config { message: format!("failed to map timer: {e}") })
    }

    fn add_gic(bus: &mut SimpleBus, config: &SocConfig) -> Result<Gic> {
        let gic = Gic::new();

        bus.add_region(MemoryRegion::new(config.gic_dist_base, 0x1000, Box::new(gic.distributor)))
            .map_err(|e| EmulatorError::Config {
                message: format!("failed to map gic distributor: {e}"),
            })?;

        bus.add_region(MemoryRegion::new(config.gic_cpu_base, 0x1000, Box::new(gic.cpu_interface)))
            .map_err(|e| EmulatorError::Config {
                message: format!("failed to map gic cpu interface: {e}"),
            })?;

        Ok(Gic::new())
    }

    pub fn minimal(ram_size: usize) -> Result<Self> {
        let config = SocConfig { ram_size, ..Default::default() };
        Self::new(config)
    }

    pub fn load_binary(&mut self, offset: u64, data: &[u8]) -> Result<()> {
        let address = self.config.ram_base + offset;
        self.bus.write_bytes(address, data).map_err(EmulatorError::Memory)?;
        info!(address = format_args!("{:#x}", address), size = data.len(), "loaded binary");
        Ok(())
    }

    pub fn step(&mut self) -> StepResult {
        if self.cpu.halted {
            return StepResult::Halted;
        }

        let pc = self.cpu.regs.pc;

        let opcode = match self.bus.read_u32(pc) {
            Ok(op) => op,
            Err(e) => return StepResult::Error(EmulatorError::Memory(e)),
        };

        trace!(pc = format_args!("{:#x}", pc), opcode = format_args!("{:#08x}", opcode), "fetch");

        let instruction = match self.decoder.decode(opcode, pc) {
            Ok(i) => i,
            Err(e) => return StepResult::Error(EmulatorError::Cpu(e)),
        };

        debug!(pc = format_args!("{:#x}", pc), instruction = ?instruction, "execute");

        match self.executor.execute(&mut self.cpu, &instruction, &mut self.bus) {
            Ok(()) => {
                self.cpu.instruction_count += 1;
                StepResult::Continue
            }
            Err(e) => self.handle_execution_error(e),
        }
    }

    fn handle_execution_error(&self, error: crate::error::CpuError) -> StepResult {
        if let crate::error::CpuError::Exception(ref msg) = error
            && let Some(imm_str) = msg.strip_prefix("breakpoint #")
            && let Ok(breakpoint_id) = imm_str.parse::<u16>()
        {
            return StepResult::Breakpoint(breakpoint_id);
        }
        StepResult::Error(EmulatorError::Cpu(error))
    }

    pub fn run(&mut self) -> Result<()> {
        let max_instructions = self.config.max_instructions;

        loop {
            if max_instructions > 0 && self.cpu.instruction_count >= max_instructions {
                info!(instructions = self.cpu.instruction_count, "reached instruction limit");
                break;
            }

            match self.step() {
                StepResult::Continue => continue,
                StepResult::Halted => {
                    info!(instructions = self.cpu.instruction_count, "cpu halted");
                    break;
                }
                StepResult::Breakpoint(id) => {
                    info!(
                        breakpoint = id,
                        instructions = self.cpu.instruction_count,
                        "breakpoint hit"
                    );
                    break;
                }
                StepResult::Error(e) => {
                    warn!(
                        error = %e,
                        pc = format_args!("{:#x}", self.cpu.regs.pc),
                        "execution error"
                    );
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    pub fn cpu_mut(&mut self) -> &mut Cpu {
        &mut self.cpu
    }

    pub fn dump_state(&self) {
        use crate::printer::Table;

        let mut table = Table::new();

        table.header("arm64 cpu state");

        table.row(&format!(
            "{}  {}  {}",
            format_reg("pc", self.cpu.regs.pc),
            format_reg("sp", self.cpu.get_sp()),
            format_reg("lr", self.cpu.regs.x[30]),
        ));

        let flags = format!(
            "{} {} {} {}  {} {}  {} {}",
            format_flag("n", self.cpu.pstate.n),
            format_flag("z", self.cpu.pstate.z),
            format_flag("c", self.cpu.pstate.c),
            format_flag("v", self.cpu.pstate.v),
            "el:".dimmed(),
            format_el(self.cpu.pstate.el),
            "sp:".dimmed(),
            if self.cpu.pstate.sp_sel { "spx" } else { "sp0" },
        );
        table.row(&flags);

        table.section("general purpose registers");

        for row in 0..11 {
            let mut parts = Vec::new();
            for col in 0..3 {
                let idx = row * 3 + col;
                if idx < 31 {
                    parts.push(format_reg(&format!("x{idx:02}"), self.cpu.regs.x[idx]));
                }
            }
            table.row(&parts.join("  "));
        }

        table.section("system registers");
        table.row(&format!(
            "{}  {}  {}",
            format_reg("vbar_el1", self.cpu.regs.vbar_el1),
            format_reg("elr_el1", self.cpu.regs.elr_el1),
            format_reg("spsr_el1", self.cpu.regs.spsr_el1),
        ));

        table.separator();
        let status = if self.cpu.halted {
            "halted".red().bold()
        } else {
            "running".green().normal()
        };
        table.row(&format!(
            "{}: {}  {}: {}",
            "instructions".dimmed(),
            format!("{}", self.cpu.instruction_count).bright_white().bold(),
            "status".dimmed(),
            status
        ));

        table.print();
    }
}

fn format_reg(name: &str, value: u64) -> String {
    let hex = format!("{value:#018x}");
    let ascii = to_ascii(value);
    if value == 0 {
        format!("{}: {} {}", name.yellow(), hex.dimmed(), ascii.dimmed())
    } else {
        format!("{}: {} {}", name.yellow(), hex.green(), ascii.bright_cyan())
    }
}

fn to_ascii(value: u64) -> String {
    value
        .to_be_bytes()
        .iter()
        .map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' })
        .collect()
}

fn format_flag(name: &str, set: bool) -> String {
    if set {
        format!("[{name}]").green().bold().to_string()
    } else {
        format!("[{name}]").dimmed().to_string()
    }
}

fn format_el(el: u8) -> String {
    match el {
        0 => "el0".bright_green().to_string(),
        1 => "el1".bright_yellow().to_string(),
        2 => "el2".bright_red().to_string(),
        3 => "el3".bright_magenta().to_string(),
        _ => "???".red().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soc_creation() {
        let soc = Soc::minimal(4096);
        assert!(soc.is_ok());
    }

    #[test]
    fn test_load_and_execute() {
        let mut soc = Soc::minimal(4096).unwrap();

        let program: &[u8] = &[0x40, 0x05, 0x80, 0xd2, 0x00, 0x00, 0x20, 0xd4];
        soc.load_binary(0, program).unwrap();

        let result = soc.step();
        assert!(matches!(result, StepResult::Continue));
        assert_eq!(soc.cpu.regs.get_x(0), 42);
    }
}
