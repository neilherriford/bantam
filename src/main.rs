use colored::Colorize;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use z80::{
    bus::{Bus, TestBus},
    cpu::Cpu,
    flags::{self, bit_is_set},
    registers::Registers,
};

#[derive(Debug, PartialEq)]
enum Fields {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
    W,
    Z,
    Ix,
    Iy,
    Sp,
    Pc,
    Cf,
    Nf,
    Pf,
    Hf,
    Zf,
    Sf,
}

impl Fields {
    pub fn flags() -> Vec<Self> {
        vec![Self::Cf, Self::Nf, Self::Pf, Self::Hf, Self::Zf, Self::Sf]
    }
}

struct State {
    pc: u16,
    sp: u16,
    cf: bool,
    nf: bool,
    pf: bool,
    hf: bool,
    zf: bool,
    sf: bool,
    a: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    w: u8,
    z: u8,
    ix: u16,
    iy: u16,
    inst: (String, Vec<Fields>),
    stack: Vec<u16>,
}

impl State {
    fn new(registers: &Registers, bus: &mut dyn Bus) -> Self {
        Self {
            pc: registers.pc,
            sp: registers.sp,
            cf: bit_is_set(registers.f, flags::CARRY),
            nf: bit_is_set(registers.f, flags::ADD_SUBTRACT),
            pf: bit_is_set(registers.f, flags::PARITY_OVERFLOW),
            hf: bit_is_set(registers.f, flags::HALF_CARRY),
            zf: bit_is_set(registers.f, flags::ZERO),
            sf: bit_is_set(registers.f, flags::SIGN),
            a: registers.a,
            b: registers.b,
            c: registers.c,
            d: registers.d,
            e: registers.e,
            h: registers.h,
            l: registers.l,
            w: registers.w,
            z: registers.z,
            ix: registers.ix,
            iy: registers.iy,
            inst: Self::decode(
                bus.read8(registers.pc),
                bus.read8(registers.pc.wrapping_add(1)),
                bus.read8(registers.pc.wrapping_add(2)),
            ),
            stack: Self::extract_stack(registers, bus),
        }
    }

    fn mark(has_changed: bool, is_effected: bool, text: &str) -> String {
        let mut result = if has_changed {
            text.truecolor(229, 0, 139)
        } else {
            text.white()
        };

        if is_effected {
            result = result.underline();
        }

        result.to_string()
    }

    pub fn header() -> String {
        format!("{}{}", Self::labels(), Self::underlines())
    }

    pub fn footer() -> String {
        format!("{}{}", Self::underlines(), Self::labels())
    }

    fn labels() -> String {
        "pc   inst            sp    C N P H Z S  a  b  c  d  e  h  l  w  z  ix   iy    stack\r\n"
            .to_string()
    }
    fn underlines() -> String {
        "▔▔▔▔ ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔ ▔▔▔▔  ▔ ▔ ▔ ▔ ▔ ▔  ▔▔ ▔▔ ▔▔ ▔▔ ▔▔ ▔▔ ▔▔ ▔▔ ▔▔ ▔▔▔▔ ▔▔▔▔  ▔▔▔▔▔\r\n"
            .to_string()
    }

    pub fn to_string(&self, previous: &State) -> String {
        let mut pc = format!("{:04X}", previous.pc);
        let mut sp = format!("{:04X}", self.sp);
        let mut cf = bit_mark(self.cf);
        let mut nf = bit_mark(self.nf);
        let mut pf = bit_mark(self.pf);
        let mut hf = bit_mark(self.hf);
        let mut zf = bit_mark(self.zf);
        let mut sf = bit_mark(self.sf);
        let mut reg_a = Self::u8_hex(self.a);
        let mut reg_b = Self::u8_hex(self.b);
        let mut reg_c = Self::u8_hex(self.c);
        let mut reg_d = Self::u8_hex(self.d);
        let mut reg_e = Self::u8_hex(self.e);
        let mut reg_h = Self::u8_hex(self.h);
        let mut reg_l = Self::u8_hex(self.l);
        let mut reg_w = Self::u8_hex(self.w);
        let mut reg_z = Self::u8_hex(self.z);
        let mut ix = format!("{:04X}", self.ix);
        let mut iy = format!("{:04X}", self.iy);

        pc = Self::mark(false, previous.inst.1.contains(&Fields::Pc), &pc);
        sp = Self::mark(
            self.sp != previous.sp,
            previous.inst.1.contains(&Fields::Sp),
            &sp,
        );
        cf = Self::mark(
            self.cf != previous.cf,
            previous.inst.1.contains(&Fields::Cf),
            &cf,
        );
        nf = Self::mark(
            self.nf != previous.nf,
            previous.inst.1.contains(&Fields::Nf),
            &nf,
        );
        pf = Self::mark(
            self.pf != previous.pf,
            previous.inst.1.contains(&Fields::Pf),
            &pf,
        );
        hf = Self::mark(
            self.hf != previous.hf,
            previous.inst.1.contains(&Fields::Hf),
            &hf,
        );
        zf = Self::mark(
            self.zf != previous.zf,
            previous.inst.1.contains(&Fields::Zf),
            &zf,
        );
        sf = Self::mark(
            self.sf != previous.sf,
            previous.inst.1.contains(&Fields::Sf),
            &sf,
        );
        reg_a = Self::mark(
            self.a != previous.a,
            previous.inst.1.contains(&Fields::A),
            &reg_a,
        );
        reg_b = Self::mark(
            self.b != previous.b,
            previous.inst.1.contains(&Fields::B),
            &reg_b,
        );
        reg_c = Self::mark(
            self.c != previous.c,
            previous.inst.1.contains(&Fields::C),
            &reg_c,
        );
        reg_d = Self::mark(
            self.d != previous.d,
            previous.inst.1.contains(&Fields::D),
            &reg_d,
        );
        reg_e = Self::mark(
            self.e != previous.e,
            previous.inst.1.contains(&Fields::E),
            &reg_e,
        );
        reg_h = Self::mark(
            self.h != previous.h,
            previous.inst.1.contains(&Fields::H),
            &reg_h,
        );
        reg_l = Self::mark(
            self.l != previous.l,
            previous.inst.1.contains(&Fields::L),
            &reg_l,
        );
        reg_w = Self::mark(
            self.w != previous.w,
            previous.inst.1.contains(&Fields::W),
            &reg_w,
        );
        reg_z = Self::mark(
            self.z != previous.z,
            previous.inst.1.contains(&Fields::Z),
            &reg_z,
        );
        ix = Self::mark(
            self.ix != previous.ix,
            previous.inst.1.contains(&Fields::Ix),
            &ix,
        );
        iy = Self::mark(
            self.iy != previous.iy,
            previous.inst.1.contains(&Fields::Iy),
            &iy,
        );

        format!(
            "{} {: <15} {}  {} {} {} {} {} {}  {} {} {} {} {} {} {} {} {} {} {}  {}\r\n",
            pc,
            previous.inst.0,
            sp,
            cf,
            nf,
            pf,
            hf,
            zf,
            sf,
            reg_a,
            reg_b,
            reg_c,
            reg_d,
            reg_e,
            reg_h,
            reg_l,
            reg_w,
            reg_z,
            ix,
            iy,
            Self::stack_to_string(previous),
        )
    }

    fn stack_to_string(previous: &State) -> String {
        let mut result = previous
            .stack
            .iter()
            .map(|value| format!("{value:04X}"))
            .collect::<Vec<String>>()
            .join(",");
        if previous.stack.len() == 5 {
            result += "…";
        }
        result
    }

    fn register_name_by_index(index: u8) -> String {
        match index {
            0 => "b",
            1 => "c",
            2 => "d",
            3 => "e",
            4 => "h",
            5 => "l",
            6 => "(HL)",
            7 => "a",
            _ => unreachable!(),
        }
        .to_string()
    }

    fn field_by_index(index: u8) -> Vec<Fields> {
        match index {
            0 => vec![Fields::B],
            1 => vec![Fields::C],
            2 => vec![Fields::D],
            3 => vec![Fields::E],
            4 => vec![Fields::H],
            5 => vec![Fields::L],
            6 => vec![Fields::H, Fields::L],
            7 => vec![Fields::A],
            _ => unreachable!(),
        }
    }

    fn condition_to_string(condition: u8) -> String {
        match condition {
            0 => "NZ",
            1 => "Z",
            2 => "NC",
            3 => "C",
            4 => "NP",
            5 => "P",
            6 => "NS",
            7 => "S",
            _ => unreachable!(),
        }
        .to_string()
    }

    fn condition_to_fields(condition: u8) -> Vec<Fields> {
        vec![match condition {
            0 | 1 => Fields::Zf,
            2 | 3 => Fields::Cf,
            4 | 5 => Fields::Pf,
            6 | 7 => Fields::Sf,
            _ => unreachable!(),
        }]
    }

    fn simple(name: &str) -> (String, Vec<Fields>) {
        (name.to_string(), vec![])
    }

    fn accumulator(name: &str) -> (String, Vec<Fields>) {
        (name.to_string(), vec![Fields::A])
    }
    fn accumulator_and_fields(name: &str, fields: Vec<Fields>) -> (String, Vec<Fields>) {
        let mut all_fields = vec![Fields::A];
        all_fields.extend(fields);
        (name.to_string(), all_fields)
    }

    fn u8_hex(value: u8) -> String {
        format!("{value:02X}")
    }

    fn u16_hex(low: u8, high: u8) -> String {
        format!("{:04X}", (u16::from(high) << 8) | u16::from(low))
    }

    fn decode(opcode: u8, next_low: u8, next_high: u8) -> (String, Vec<Fields>) {
        let group = opcode >> 6;
        let operand1 = (opcode >> 3) & 7;
        let operand2 = opcode & 7;

        match (group, operand1, operand2) {
            (0, 0, 0) => Self::simple("NOP"),
            (1, 6, 6) => Self::simple("HALT"),
            (3, 6, 3) => Self::simple("DI"),
            (3, 7, 3) => Self::simple("EI"),
            (1, dest, src) => (
                format!(
                    "LD {}, {}",
                    Self::register_name_by_index(dest),
                    Self::register_name_by_index(src)
                ),
                Self::field_by_index(dest)
                    .into_iter()
                    .chain(Self::field_by_index(src))
                    .collect(),
            ),
            (0, register, 6) => (
                format!(
                    "LD {}, ${}",
                    Self::register_name_by_index(register),
                    Self::u8_hex(next_low)
                ),
                Self::field_by_index(register),
            ),
            (0, op @ 0..=3, 2) => {
                let address;
                let fields;
                if op & 2 == 0 {
                    address = "(BC)";
                    fields = vec![Fields::B, Fields::C];
                } else {
                    address = "(DE)";
                    fields = vec![Fields::D, Fields::E];
                }

                if op & 1 == 0 {
                    (format!("LD {address}, A"), fields)
                } else {
                    (format!("LD A, {address}"), fields)
                }
            }
            (0, op @ 4..=7, 2) => {
                if op & 2 == 0 {
                    if op & 1 == 0 {
                        (
                            format!("LD (${}), HL", Self::u16_hex(next_low, next_high)),
                            vec![Fields::H, Fields::L],
                        )
                    } else {
                        (
                            format!("LD HL, (${})", Self::u16_hex(next_low, next_high)),
                            vec![Fields::H, Fields::L],
                        )
                    }
                } else if op & 1 == 0 {
                    (
                        format!("LD (${}), A", Self::u16_hex(next_low, next_high)),
                        vec![Fields::A],
                    )
                } else {
                    (
                        format!("LD A, (${})", Self::u16_hex(next_low, next_high)),
                        vec![Fields::A],
                    )
                }
            }
            (0, pair @ (0 | 2 | 4 | 6), 1) => {
                let dest = match pair {
                    0 => "BC",
                    2 => "DE",
                    4 => "HL",
                    6 => "SP",
                    _ => unreachable!(),
                };
                let fields = match pair {
                    0 => vec![Fields::B, Fields::C],
                    2 => vec![Fields::D, Fields::E],
                    4 => vec![Fields::H, Fields::L],
                    6 => vec![Fields::Sp],
                    _ => unreachable!(),
                };

                (
                    format!("LD {dest}, ${}", Self::u16_hex(next_low, next_high)),
                    fields,
                )
            }
            (3, pair @ (0 | 2 | 4 | 6), 5) => {
                let arg = match pair {
                    0 => "BC",
                    2 => "DE",
                    4 => "HL",
                    6 => "AF",
                    _ => unreachable!(),
                };

                let fields = match pair {
                    0 => vec![Fields::B, Fields::C],
                    2 => vec![Fields::D, Fields::E],
                    4 => vec![Fields::H, Fields::L],
                    6 => vec![
                        Fields::A,
                        Fields::Cf,
                        Fields::Nf,
                        Fields::Pf,
                        Fields::Hf,
                        Fields::Zf,
                        Fields::Sf,
                    ],
                    _ => unreachable!(),
                };
                (format!("PUSH {arg}"), fields)
            }
            (3, pair @ (0 | 2 | 4 | 6), 1) => {
                let arg = match pair {
                    0 => "BC",
                    2 => "DE",
                    4 => "HL",
                    6 => "AF",
                    _ => unreachable!(),
                };
                let fields = match pair {
                    0 => vec![Fields::B, Fields::C],
                    2 => vec![Fields::D, Fields::E],
                    4 => vec![Fields::H, Fields::L],
                    6 => vec![
                        Fields::A,
                        Fields::Cf,
                        Fields::Nf,
                        Fields::Pf,
                        Fields::Hf,
                        Fields::Zf,
                        Fields::Sf,
                    ],
                    _ => unreachable!(),
                };
                (format!("POP {arg}"), fields)
            }
            (0, register, 4) => (
                format!("INC {}", Self::register_name_by_index(register)),
                Self::field_by_index(register),
            ),
            (0, register, 5) => (
                format!("DEC {}", Self::register_name_by_index(register)),
                Self::field_by_index(register),
            ),
            (2, 0, register) => (
                format!("ADD A, {}", Self::register_name_by_index(register)),
                Self::field_by_index(register),
            ),
            (2, 1, register) => (
                format!("ADC A, {}", Self::register_name_by_index(register)),
                Self::field_by_index(register),
            ),
            (3, 0, 6) => Self::accumulator("ADD A, n"),
            (3, 1, 6) => Self::accumulator("ADC A, n"),
            (2, 2, register) => Self::accumulator_and_fields(
                &format!("SUB A, {}", Self::register_name_by_index(register)),
                Self::field_by_index(register),
            ),
            (2, 3, register) => Self::accumulator_and_fields(
                &format!("SBC A, {}", Self::register_name_by_index(register)),
                Self::field_by_index(register),
            ),
            (3, 2, 6) => Self::accumulator("SUB A, n"),
            (3, 3, 6) => Self::accumulator("SBC A, n"),
            (2, 7, register) => Self::accumulator_and_fields(
                &format!("CP {}", Self::register_name_by_index(register)),
                Self::field_by_index(register),
            ),
            (3, 7, 6) => Self::accumulator(&format!("CP ${}", Self::u8_hex(next_low))),
            (2, 4, register) => Self::accumulator_and_fields(
                &format!("AND {}", Self::register_name_by_index(register)),
                Self::field_by_index(register),
            ),
            (2, 6, register) => Self::accumulator_and_fields(
                &format!("OR {}", Self::register_name_by_index(register)),
                Self::field_by_index(register),
            ),
            (2, 5, register) => Self::accumulator_and_fields(
                &format!("XOR {}", Self::register_name_by_index(register)),
                Self::field_by_index(register),
            ),
            (3, 4, 6) => Self::accumulator(&format!("AND ${}", Self::u8_hex(next_low))),
            (3, 6, 6) => Self::accumulator(&format!("OR ${}", Self::u8_hex(next_low))),
            (3, 5, 6) => Self::accumulator(&format!("XOR ${}", Self::u8_hex(next_low))),
            (0, pair @ (1 | 3 | 5 | 7), 1) => {
                let source = match pair {
                    1 => "bc",
                    3 => "de",
                    5 => "hl",
                    7 => "sp",
                    _ => unreachable!(),
                };
                (format!("ADD HL, {source}"), vec![Fields::H, Fields::L])
            }
            (3, 0, 3) => (
                format!("JP ${}", Self::u16_hex(next_low, next_high)),
                vec![Fields::Pc],
            ),
            (0, 3, 0) => (format!("JR ${}", Self::u8_hex(next_low)), vec![Fields::Pc]),
            (0, 4, 0) => (
                format!("JR NZ, ${}", Self::u8_hex(next_low)),
                vec![Fields::Pc, Fields::Zf],
            ),
            (0, 5, 0) => (
                format!("JR Z, ${}", Self::u8_hex(next_low)),
                vec![Fields::Pc, Fields::Zf],
            ),
            (0, 6, 0) => (
                format!("JR NC, ${}", Self::u8_hex(next_low)),
                vec![Fields::Pc, Fields::Cf],
            ),
            (0, 7, 0) => (
                format!("JR C, ${}", Self::u8_hex(next_low)),
                vec![Fields::Pc, Fields::Cf],
            ),
            (3, 5, 1) => ("JP (HL)".to_string(), vec![Fields::Pc]),
            (3, condition, 2) => (
                format!(
                    "JP {} ${}",
                    Self::condition_to_string(condition),
                    Self::u16_hex(next_low, next_high)
                ),
                vec![Fields::Pc, Fields::H, Fields::L],
            ),
            (3, 1, 5) => (
                format!("CALL ${}", Self::u16_hex(next_low, next_high)),
                vec![Fields::Pc],
            ),
            (3, condition, 4) => (
                format!(
                    "CALL {} ${}",
                    Self::condition_to_string(condition),
                    Self::u16_hex(next_low, next_high)
                ),
                Self::condition_to_fields(condition)
                    .into_iter()
                    .chain(vec![Fields::Pc])
                    .collect(),
            ),
            (3, 1, 1) => ("RET".to_string(), vec![Fields::Pc]),
            (3, condition, 0) => (
                format!("RET {}", Self::condition_to_string(condition)),
                Self::condition_to_fields(condition)
                    .into_iter()
                    .chain(vec![Fields::Pc])
                    .collect(),
            ),
            (3, address_shorthand, 7) => {
                let address = match address_shorthand {
                    0 => 0x00,
                    1 => 0x08,
                    2 => 0x10,
                    3 => 0x18,
                    4 => 0x20,
                    5 => 0x28,
                    6 => 0x30,
                    7 => 0x38,
                    _ => unreachable!(),
                };
                (format!("RST {address}"), vec![Fields::Pc])
            }
            (3, 3, 3) => Self::accumulator("IN A, (n)"),
            (3, 2, 3) => Self::accumulator("OUT n, A"),
            (3, 5, 3) => (
                "EX DE, HL".to_string(),
                vec![Fields::D, Fields::E, Fields::H, Fields::L],
            ),
            (0, 1, 0) => Self::accumulator_and_fields("EX AF, AF'", Fields::flags()),
            (3, 3, 1) => (
                "EXX".to_string(),
                vec![Fields::B, Fields::C, Fields::D, Fields::E],
            ),
            (3, 4, 3) => (
                "EX (SP), HL".to_string(),
                vec![Fields::Sp, Fields::H, Fields::L],
            ),
            (0, 0, 7) => Self::accumulator("RLCA"),
            (0, 1, 7) => Self::accumulator("RRCA"),
            (0, 2, 7) => Self::accumulator("RLA"),
            (0, 3, 7) => Self::accumulator("RRA"),
            (0, 4, 7) => Self::accumulator("DAA"),
            (0, 5, 7) => Self::accumulator("CPL"),
            (0, 6, 7) => ("SCF".to_string(), vec![Fields::Cf]),
            (0, 7, 7) => ("CCF".to_string(), vec![Fields::Cf]),
            (0, 2, 0) => ("DJNZ".to_string(), vec![Fields::Pc, Fields::B]),
            _ => panic!("Unsupported instruction"),
        }
    }

    fn extract_stack(registers: &Registers, bus: &mut dyn Bus) -> Vec<u16> {
        let mut index = registers.sp;
        let mut result = vec![];

        while index <= 0xFFFD {
            if result.len() == 5 {
                break;
            }
            let low = bus.read8(index);
            let high = bus.read8(index + 1);
            result.push(u16::from(high) << 8 | u16::from(low));
            index += 2;
        }

        result
    }
}

fn bit_mark(set: bool) -> String {
    if set { "×" } else { "·" }.to_string()
}

fn main() {
    const ROM: &[u8] = include_bytes!("../z80/tools/sjasmplus/fib.bin");

    let mut cpu = Cpu::new(Registers::new(), TestBus::new());
    cpu.bus.load(ROM);
    let mut previous_state;
    let mut current_state = State::new(&cpu.registers, &mut cpu.bus);

    print!("{}", State::header());
    let _ = enable_raw_mode();
    let mut step = true;

    while !cpu.registers.halted {
        cpu.step();
        previous_state = current_state;
        current_state = State::new(&cpu.registers, &mut cpu.bus);
        print!("{}", current_state.to_string(&previous_state));
        if step {
            step = wait_for_key_press();
        }
    }
    print!("{}", State::footer());
    let _ = disable_raw_mode();
}

fn wait_for_key_press() -> bool {
    loop {
        if let Ok(Event::Key(key)) = event::read() {
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                let _ = disable_raw_mode();
                std::process::exit(0);
            }
            return key.code != KeyCode::Char('r');
        }
    }
}
