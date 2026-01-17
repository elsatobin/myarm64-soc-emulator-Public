use arm64_soc_emulator::system::{Soc, StepResult};

const SIMPLE_PROGRAM: &[u8] = &[0x40, 0x05, 0x80, 0xd2, 0x00, 0x00, 0x40, 0xd4];

#[test]
fn test_simple_mov_hlt() {
    let mut soc = Soc::minimal(4096).unwrap();
    soc.load_binary(0, SIMPLE_PROGRAM).unwrap();

    loop {
        match soc.step() {
            StepResult::Continue => continue,
            StepResult::Halted => break,
            StepResult::Breakpoint(_) => break,
            StepResult::Error(e) => panic!("execution error: {e}"),
        }
    }

    assert_eq!(soc.cpu().regs.get_x(0), 42);
    assert!(soc.cpu().halted);
}

#[test]
fn test_instruction_count() {
    let mut soc = Soc::minimal(4096).unwrap();
    soc.load_binary(0, SIMPLE_PROGRAM).unwrap();
    soc.run().unwrap();

    assert_eq!(soc.cpu().instruction_count, 2);
}

#[test]
fn test_conditional_branch() {
    let program: &[u8] = &[
        0x02, 0x00, 0x80, 0xd2, 0x41, 0x00, 0x80, 0xd2, 0x42, 0x04, 0x00, 0x91, 0x21, 0x04, 0x00,
        0xf1, 0xc1, 0xff, 0xff, 0x54, 0x00, 0x00, 0x40, 0xd4,
    ];

    let mut soc = Soc::minimal(4096).unwrap();
    soc.load_binary(0, program).unwrap();
    soc.run().unwrap();

    assert_eq!(soc.cpu().regs.get_x(2), 2);
}
