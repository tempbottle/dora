use std::mem;

use dora_parser::ast::{self, Ast};
use ctxt::{SemContext, Fct, FctId};
use driver::cmd;
use dora_parser::error::msg::Msg;

use dora_parser::interner::Interner;
use dora_parser::lexer::reader::Reader;
use baseline;
use dora_parser::lexer::position::Position;
use os;

use dora_parser::parser::{Parser, NodeIdGenerator};
use semck;
use stacktrace::DoraToNativeInfo;
use ty::BuiltinType;

pub fn start() -> i32 {
    let args = cmd::parse();

    if args.flag_version {
        println!("dora v0.01b");
        return 0;
    }

    let mut interner = Interner::new();
    let id_generator = NodeIdGenerator::new();
    let mut ast = Ast::new();

    if let Err(code) = parse_file("stdlib/prelude.dora",
                                  &id_generator,
                                  &mut ast,
                                  &mut interner)
               .and_then(|_| {
                             parse_file("stdlib/str.dora", &id_generator, &mut ast, &mut interner)
                         })
               .and_then(|_| {
                             parse_file("stdlib/io.dora", &id_generator, &mut ast, &mut interner)
                         })
               .and_then(|_| {
                             parse_file("stdlib/utils.dora", &id_generator, &mut ast, &mut interner)
                         })
               .and_then(|_| {
                             parse_file(&args.arg_file, &id_generator, &mut ast, &mut interner)
                         }) {
        return code;
    }

    if args.flag_emit_ast {
        ast::dump::dump(&ast, &interner);
    }

    let mut ctxt = SemContext::new(args, &ast, interner);

    semck::check(&mut ctxt);

    // register signal handler
    os::register_signals(&ctxt);

    let main = if ctxt.args.cmd_test {
        None
    } else {
        find_main(&ctxt)
    };

    if ctxt.diag.borrow().has_errors() {
        ctxt.diag.borrow().dump();
        let no_errors = ctxt.diag.borrow().errors().len();

        if no_errors == 1 {
            println!("{} error found.", no_errors);
        } else {
            println!("{} errors found.", no_errors);
        }

        return 1;
    }

    if ctxt.args.cmd_test {
        run_tests(&ctxt)
    } else {
        run_main(&ctxt, main.unwrap())
    }
}

fn run_tests<'ast>(ctxt: &SemContext<'ast>) -> i32 {
    let mut tests = 0;
    let mut passed = 0;

    for fct in ctxt.fcts.iter() {
        let fct = fct.borrow();

        if !is_test_fct(ctxt, &*fct) {
            continue;
        }

        tests += 1;

        print!("test {} ... ", ctxt.interner.str(fct.name));

        if run_test(ctxt, fct.id) {
            passed += 1;
            println!("ok");
        } else {
            println!("failed");
        }
    }

    println!("{} tests executed; {} passed; {} failed.",
             tests,
             passed,
             tests - passed);

    1
}

fn run_test<'ast>(ctxt: &SemContext<'ast>, fct: FctId) -> bool {
    let fct_ptr = {
        let mut sfi = DoraToNativeInfo::new();

        ctxt.use_sfi(&mut sfi, || baseline::generate(&ctxt, fct))
    };

    let fct: extern "C" fn() -> i32 = unsafe { mem::transmute(fct_ptr) };
    let res = fct();

    true
}

fn is_test_fct<'ast>(ctxt: &SemContext<'ast>, fct: &Fct<'ast>) -> bool {
    if !fct.parent.is_none() || !fct.return_type.is_unit() || fct.param_types.len() != 0 {
        return false;
    }

    let fct_name = ctxt.interner.str(fct.name);
    fct_name.starts_with("test")
}

fn run_main<'ast>(ctxt: &SemContext<'ast>, main: FctId) -> i32 {
    let fct_ptr = {
        let mut sfi = DoraToNativeInfo::new();

        ctxt.use_sfi(&mut sfi, || baseline::generate(&ctxt, main))
    };

    let fct: extern "C" fn() -> i32 = unsafe { mem::transmute(fct_ptr) };
    let res = fct();

    let is_unit = ctxt.fcts[main].borrow().return_type.is_unit();

    // main-fct without return value exits with status 0
    if is_unit {
        0

        // else use return value of main for exit status
    } else {
        res
    }
}

fn parse_file(filename: &str,
              id_generator: &NodeIdGenerator,
              ast: &mut Ast,
              interner: &mut Interner)
              -> Result<(), i32> {
    let reader = if filename == "-" {
        match Reader::from_input() {
            Err(_) => {
                println!("unable to read from stdin.");
                return Err(1);
            }

            Ok(reader) => reader,
        }
    } else {
        match Reader::from_file(filename) {
            Err(_) => {
                println!("unable to read file `{}`", filename);
                return Err(1);
            }

            Ok(reader) => reader,
        }
    };

    if let Err(error) = Parser::new(reader, id_generator, ast, interner).parse() {
        println!("{}", error);
        println!("1 error found.");
        return Err(1);
    }

    Ok(())
}

fn find_main<'ast>(ctxt: &SemContext<'ast>) -> Option<FctId> {
    let name = ctxt.interner.intern("main");
    let fctid = match ctxt.sym.borrow().get_fct(name) {
        Some(id) => id,
        None => {
            ctxt.diag
                .borrow_mut()
                .report(Position::new(1, 1), Msg::MainNotFound);
            return None;
        }
    };

    let fct = ctxt.fcts[fctid].borrow();
    let ret = fct.return_type;

    if (ret != BuiltinType::Unit && ret != BuiltinType::Int) ||
       fct.params_without_self().len() > 0 {
        let pos = fct.ast.pos;
        ctxt.diag
            .borrow_mut()
            .report(pos, Msg::WrongMainDefinition);
        return None;
    }

    Some(fctid)
}
