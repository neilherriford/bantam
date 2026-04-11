#[inline]
pub fn into_group_and_operands(opcode: u8) -> (u8, u8, u8) {
    let group = opcode >> 6;
    let operand1 = (opcode >> 3) & 7;
    let operand2 = opcode & 7;

    (group, operand1, operand2)
}
