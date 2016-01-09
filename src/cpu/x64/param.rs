use ast::Expr;
use cpu;
use sym::BuiltinType;

// first param offset to rbp is +16,
// rbp+0 -> saved rbp
// rbp+8 -> return address
pub static PARAM_OFFSET: i32 = 16;

// on x64 each parameter needs exactly 8 bytes
pub fn next_param_offset(param_offset: i32, _: BuiltinType) -> i32 {
    param_offset + 8
}

// needed memory on stack for function call in caller
pub fn reserve_stack_for_call(args: &[Box<Expr>]) -> u32 {
    // some function parameters are stored on the stack,
    // therefore we need to increase `tempsize` in this case.
    let params_on_stack = args.len() as i32 - cpu::REG_PARAMS.len() as i32;

    if params_on_stack > 0 {
        (params_on_stack as u32) * 8
    } else {
        0
    }
}